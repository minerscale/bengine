use std::sync::Arc;

use ash::vk;
use tracing_mutex::stdsync::Mutex;
use ultraviolet::{Isometry3, Lerp, Rotor3, Slerp, Vec2, Vec3};

use crate::{
    FOV,
    audio::{Audio, AudioParameters},
    clock::{Clock, FIXED_UPDATE_INTERVAL},
    event_loop::Input,
    node::{Node, Object},
    physics::{Physics, from_nalgebra},
    player::Player,
    renderer::{
        Renderer, UniformBufferObject, buffer::MappedBuffer,
        command_buffer::ActiveMultipleSubmitCommandBuffer, device::Device, image::SwapchainImage,
        render_pass::RenderPass,
    },
    scene::create_scene,
    skybox::Skybox,
};

pub struct Game {
    player: Player,
    physics: Physics,
    audio: Audio,
    scene: Vec<Node>,
    clock: Clock,
    skybox: Skybox,
}

impl Game {
    fn get_camera_rotor(camera_rotation: Vec2) -> Rotor3 {
        Rotor3::from_rotation_xz(camera_rotation.x) * Rotor3::from_rotation_yz(camera_rotation.y)
    }

    pub fn new(gfx: &Renderer) -> Self {
        let mut physics = Physics::new();
        let player = Player::new(&mut physics);
        let audio = Audio::new();
        let scene = create_scene(&gfx, &mut physics);
        let clock = Clock::new();
        let skybox = Skybox::new(&gfx);

        Self {
            player,
            physics,
            audio,
            scene,
            clock,
            skybox,
        }
    }

    pub fn draw(
        &mut self,
        input: Arc<Mutex<Input>>,
        device: &Device,
        render_pass: &RenderPass,
        command_buffer: ActiveMultipleSubmitCommandBuffer,
        uniform_buffers: &mut [MappedBuffer<UniformBufferObject>],
        image: &SwapchainImage,
        extent: &vk::Extent2D,
    ) -> ActiveMultipleSubmitCommandBuffer {
        let interpolation_factor = ((std::time::Instant::now() - self.clock.previous_time)
            .as_secs_f64()
            / FIXED_UPDATE_INTERVAL) as f32;

        let player_transform = self
            .player
            .previous_position
            .lerp(self.player.position, interpolation_factor);

        let minput = input.lock().unwrap();
        let camera_rotation = Self::get_camera_rotor(
            minput
                .previous
                .camera_rotation
                .lerp(minput.camera_rotation, interpolation_factor),
        );
        drop(minput);

        let camera_transform = Isometry3::new(
            player_transform + Vec3::new(0.0, 0.8, 0.0),
            camera_rotation.reversed(),
        );

        let fov = FOV.to_radians();
        let ez = f32::tan(fov / 2.0).recip();

        let ubo = UniformBufferObject {
            view_transform: camera_transform,
            time: self.clock.time,
            fov: ez,
            scale_y: (extent.width as f32) / (extent.height as f32),
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
                &render_pass.pipelines[1],
                &uniform_buffer.descriptor_set,
            );

            let pipeline = &render_pass.pipelines[0];
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

            device.cmd_end_render_pass(cmd_buf);

            command_buffer
        }
    }

    pub fn update(&mut self, input: Arc<Mutex<Input>>) {
        self.clock.update();
        let dt = self.clock.dt;

        let player_rigid_body_handle = self.player.rigid_body_handle;

        let input = input.lock().unwrap();

        let player = &mut self.player;
        let physics = &mut self.physics;

        player.update(
            physics,
            &input,
            Self::get_camera_rotor(input.camera_rotation),
            dt,
        );

        let player_transform =
            from_nalgebra(physics.rigid_body_set[player_rigid_body_handle].position());

        drop(input);

        physics.step(&mut self.scene, &mut self.player, dt);

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
