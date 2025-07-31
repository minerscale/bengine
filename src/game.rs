use ash::vk;
use easy_cast::{Cast, CastApprox};
use tracing_mutex::stdsync::Mutex;
use ultraviolet::{Isometry3, Lerp, Rotor3, Slerp, Vec2, Vec3};

use crate::{
    FOV,
    audio::{Audio, AudioParameters},
    clock::{Clock, FIXED_UPDATE_INTERVAL},
    event_loop::SharedState,
    gui::create_gui,
    gui::egui_backend::EguiBackend,
    node::{Node, Object},
    physics::{Physics, from_nalgebra},
    player::Player,
    renderer::{
        Renderer, UniformBufferObject, buffer::MappedBuffer,
        command_buffer::ActiveMultipleSubmitCommandBuffer, device::Device, image::SwapchainImage,
        render_pass::RenderPass,
    },
    scene::create_scene,
    shader_pipelines::{EGUI_PIPELINE, MAIN_PIPELINE, SKYBOX_PIPELINE},
    skybox::Skybox,
};

pub struct Game {
    pub player: Player,
    pub physics: Physics,
    pub audio: Audio,
    pub scene: Vec<Node>,
    pub clock: Clock,
    pub skybox: Skybox,
    pub gui: EguiBackend,
}

impl Game {
    fn get_camera_rotor(camera_rotation: Vec2) -> Rotor3 {
        Rotor3::from_rotation_xz(camera_rotation.x) * Rotor3::from_rotation_yz(camera_rotation.y)
    }

    pub fn new(gfx: &Renderer) -> Self {
        let mut physics = Physics::new();
        let player = Player::new(&mut physics);
        let audio = Audio::new();
        let scene = create_scene(gfx, &mut physics);
        let clock = Clock::new();
        let skybox = Skybox::new(gfx);

        let gui = EguiBackend::new(gfx, create_gui());

        Self {
            player,
            physics,
            audio,
            scene,
            clock,
            skybox,
            gui,
        }
    }

    pub fn draw(
        &mut self,
        shared_state: &Mutex<SharedState>,
        device: &Device,
        render_pass: &RenderPass,
        command_buffer: ActiveMultipleSubmitCommandBuffer,
        uniform_buffers: &mut [MappedBuffer<UniformBufferObject>],
        image: &SwapchainImage,
    ) -> ActiveMultipleSubmitCommandBuffer {
        let window_size = egui::Vec2::new(image.extent.width.cast(), image.extent.height.cast());
        self.gui.window_size = window_size;

        let interpolation_factor = (self.clock.previous_time.elapsed().as_secs_f64()
            / FIXED_UPDATE_INTERVAL)
            .cast_approx();

        let player_transform = self
            .player
            .previous_position
            .lerp(self.player.position, interpolation_factor);

        let state = shared_state.lock().unwrap();
        let camera_rotation = Self::get_camera_rotor(
            state
                .previous
                .camera_rotation
                .lerp(state.camera_rotation, interpolation_factor),
        );
        drop(state);

        let camera_transform = Isometry3::new(
            player_transform + Vec3::new(0.0, 0.8, 0.0),
            camera_rotation.reversed(),
        );

        let fov = FOV.to_radians();
        let ez = f32::tan(fov / 2.0).recip();

        let ubo = UniformBufferObject {
            view_transform: camera_transform,
            time: self.clock.time.cast_approx(),
            fov: ez,
            scale_y: window_size.x / window_size.y,
        };

        let clear_color = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            },
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [1.0, 0.0, 0.0, 0.0],
                },
            },
        ];

        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(**render_pass)
            .framebuffer(image.framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: image.extent,
            })
            .clear_values(&clear_color);

        let uniform_buffer = &mut uniform_buffers[0];

        let uniform_buffer_descriptor_set = [*uniform_buffer.descriptor_set];
        let ubo_mapped = uniform_buffer.mapped_memory.first_mut().unwrap();
        *ubo_mapped = ubo;

        let cmd_buf = *command_buffer;

        let command_buffer =
            self.skybox
                .render(device, command_buffer, &uniform_buffer.descriptor_set);

        unsafe {
            device.cmd_begin_render_pass(cmd_buf, &render_pass_info, vk::SubpassContents::INLINE);

            let command_buffer = self.skybox.blit(
                device,
                command_buffer,
                &render_pass.pipelines[SKYBOX_PIPELINE],
                &uniform_buffer.descriptor_set,
            );

            let pipeline = &render_pass.pipelines[MAIN_PIPELINE];
            device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, **pipeline);
            device.cmd_bind_descriptor_sets(
                cmd_buf,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.pipeline_layout,
                0,
                &uniform_buffer_descriptor_set,
                &[],
            );

            for node in &self.scene {
                fn interpolate_isometry(a: Isometry3, b: Isometry3, t: f32) -> Isometry3 {
                    Isometry3::new(
                        a.translation.lerp(b.translation, t),
                        a.rotation.slerp(b.rotation, t).normalized(),
                    )
                }

                let transform = interpolate_isometry(
                    node.previous_transform,
                    node.transform,
                    interpolation_factor,
                );

                let modelview_transform = Isometry3 {
                    translation: (transform.translation - ubo.view_transform.translation)
                        .rotated_by(ubo.view_transform.rotation),
                    rotation: ubo.view_transform.rotation * transform.rotation,
                };

                for object in &node.objects {
                    if let Object::Mesh(mesh) = object {
                        mesh.draw(device, cmd_buf, pipeline, modelview_transform);
                    }
                }
            }

            let pipeline = &render_pass.pipelines[EGUI_PIPELINE];
            self.gui.draw(device, image.extent, cmd_buf, pipeline);

            device.cmd_end_render_pass(cmd_buf);

            command_buffer
        }
    }

    pub fn update(
        &mut self,
        shared_state: &Mutex<SharedState>,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) {
        self.clock.update();

        self.gui.update_input(&self.clock, events, modifiers);

        let player_rigid_body_handle = self.player.rigid_body_handle;

        let state = shared_state.lock().unwrap();

        let player = &mut self.player;
        let physics = &mut self.physics;

        player.update(
            physics,
            &state,
            Self::get_camera_rotor(state.camera_rotation),
            self.clock.dt,
        );

        let player_transform =
            from_nalgebra(physics.rigid_body_set[player_rigid_body_handle].position());

        drop(state);

        physics.step(&mut self.scene, &mut self.player, self.clock.dt);

        let gems_and_jewel_location = Vec2::new(8.0, 8.0);
        let distance = (Vec2::new(
            player_transform.translation.x,
            player_transform.translation.z,
        ) - gems_and_jewel_location)
            .mag();

        self.audio
            .parameter_stream
            .send(AudioParameters::new(distance.into()))
            .unwrap();
    }
}
