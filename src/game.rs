use std::sync::Arc;

use ash::vk;
use easy_cast::{Cast, CastApprox, CastFloat};
use libpd_rs::Pd;
use log_once::warn_once;
use ultraviolet::{Isometry3, Lerp, Rotor3, Slerp, Vec2, Vec3};

use crate::{
    FOV,
    audio::Audio,
    clock::{Clock, FIXED_UPDATE_INTERVAL},
    event_loop::SharedState,
    gltf::{GltfFile, load_gltf},
    gui::{create_gui, egui_backend::EguiBackend},
    mesh::Mesh,
    node::{Behaviour, Node, Object},
    physics::{Physics, from_nalgebra},
    player::Player,
    renderer::{
        Renderer, UniformBufferObject,
        buffer::MappedBuffer,
        command_buffer::{ActiveMultipleSubmitCommandBuffer, OneTimeSubmitCommandBuffer},
        device::Device,
        image::{Image, SwapchainImage},
        material::{Material, MaterialProperties},
        render_pass::RenderPass,
        sampler::Sampler,
    },
    scene::create_scene,
    shader_pipelines::{EGUI_PIPELINE, MAIN_PIPELINE, MATERIAL_LAYOUT, SKYBOX_PIPELINE},
    skybox::Skybox,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum GameState {
    Menu,
    Playing,
}

impl From<GameState> for &str {
    fn from(value: GameState) -> Self {
        match value {
            GameState::Menu => "menu",
            GameState::Playing => "playing",
        }
    }
}

pub struct Game {
    pub player: Player,
    pub physics: Physics,
    pub audio: Audio,
    pub scene: Vec<Node>,
    metal_detector_objects: Vec<MetalDetectorObject>,
    default_material: Arc<Material>,
    pub clock: Clock,
    pub skybox: Skybox,
    pub gui: EguiBackend,
}

impl Game {
    fn get_camera_rotor(camera_rotation: Vec2) -> Rotor3 {
        Rotor3::from_rotation_xz(camera_rotation.x) * Rotor3::from_rotation_yz(camera_rotation.y)
    }

    pub fn new(gfx: &Renderer, pd: &mut Pd) -> Self {
        let mut physics = Physics::new();
        let player = Player::new(&mut physics);
        let audio = Audio::new(pd);
        let scene = create_scene(gfx, &mut physics);
        let clock = Clock::new();
        let skybox = Skybox::new(gfx);

        let gui = EguiBackend::new(gfx, create_gui());

        let (metal_detector_objects, default_image) =
            gfx.command_pool
                .one_time_submit(gfx.device.graphics_queue, |cmd_buf| {
                    (
                        METAL_DETECTOR_MANIFESTS
                            .into_iter()
                            .map(|obj| obj.into_metal_detector_object(gfx, cmd_buf))
                            .collect(),
                        Image::from_image(
                            &gfx.device,
                            cmd_buf,
                            image::load_from_memory(include_bytes!(
                                "../test-objects/middle-grey.png"
                            ))
                            .unwrap(),
                            true,
                        ),
                    )
                });

        let default_material = Arc::new(Material::new(
            &gfx.device,
            default_image,
            Sampler::default(gfx.device.clone()).into(),
            MaterialProperties { alpha_cutoff: 0.0 },
            &gfx.descriptor_pool,
            &gfx.descriptor_set_layouts[MATERIAL_LAYOUT],
        ));

        Self {
            player,
            physics,
            audio,
            scene,
            metal_detector_objects,
            default_material,
            clock,
            skybox,
            gui,
        }
    }

    fn begin_render_pass(
        device: &Device,
        command_buffer: ActiveMultipleSubmitCommandBuffer,
        render_pass: &RenderPass,
        image: &SwapchainImage,
    ) -> ActiveMultipleSubmitCommandBuffer {
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

        unsafe {
            device.cmd_begin_render_pass(
                *command_buffer,
                &render_pass_info,
                vk::SubpassContents::INLINE,
            );
        }

        command_buffer
    }

    fn draw_playing(
        &mut self,
        shared_state: &mut SharedState,
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

        let camera_rotation = Self::get_camera_rotor(
            shared_state
                .previous
                .camera_rotation
                .lerp(shared_state.camera_rotation, interpolation_factor),
        );

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

        let uniform_buffer = &mut uniform_buffers[0];

        let uniform_buffer_descriptor_set = [*uniform_buffer.descriptor_set];
        let ubo_mapped = uniform_buffer.mapped_memory.first_mut().unwrap();
        *ubo_mapped = ubo;

        let cmd_buf = *command_buffer;

        let command_buffer =
            self.skybox
                .render(device, command_buffer, &uniform_buffer.descriptor_set);

        let command_buffer = Self::begin_render_pass(device, command_buffer, render_pass, image);

        unsafe {
            let mut command_buffer = self.skybox.blit(
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
                        mesh.draw(
                            device,
                            &mut command_buffer,
                            pipeline,
                            modelview_transform,
                            &self.default_material,
                        );
                    }
                }
            }

            command_buffer
        }
    }

    pub fn draw(
        &mut self,
        shared_state: &mut SharedState,
        device: &Device,
        render_pass: &RenderPass,
        command_buffer: ActiveMultipleSubmitCommandBuffer,
        uniform_buffers: &mut [MappedBuffer<UniformBufferObject>],
        image: &SwapchainImage,
    ) -> ActiveMultipleSubmitCommandBuffer {
        let command_buffer = if shared_state.game_state() == GameState::Playing {
            self.draw_playing(
                shared_state,
                device,
                render_pass,
                command_buffer,
                uniform_buffers,
                image,
            )
        } else {
            Self::begin_render_pass(device, command_buffer, render_pass, image)
        };

        let cmd_buf = *command_buffer;
        let pipeline = &render_pass.pipelines[EGUI_PIPELINE];

        self.gui.draw(device, image.extent, cmd_buf, pipeline);

        unsafe { device.cmd_end_render_pass(cmd_buf) };

        command_buffer
    }

    fn update_playing(&mut self, pd: &mut Pd, input: &mut SharedState) {
        let player_rigid_body_handle = self.player.rigid_body_handle;

        let player = &mut self.player;
        let physics = &mut self.physics;

        player.update(
            physics,
            input,
            Self::get_camera_rotor(input.camera_rotation),
            self.clock.dt,
        );

        let player_transform =
            from_nalgebra(physics.rigid_body_set[player_rigid_body_handle].position());

        physics.step(&mut self.scene, &mut self.player, self.clock.dt);

        let player_xz = Vec2::new(
            player_transform.translation.x,
            player_transform.translation.z,
        );

        let distance = |a: &MetalDetectorObject| (player_xz - a.location).mag();
        let distance_tuple = |a: (usize, &MetalDetectorObject)| distance(a.1);

        let badness = self
            .metal_detector_objects
            .iter()
            .fold(0.0, |acc, obj| acc + obj.badness / (distance(obj) + 1.0));

        let closest_object = self
            .metal_detector_objects
            .iter()
            .enumerate()
            .min_by(|&a, &b| distance_tuple(a).total_cmp(&distance_tuple(b)));

        let distance = closest_object.map_or(f32::MAX, distance_tuple);

        // Dig up an object
        if let Some((idx, object)) = closest_object
            && input.action()
            && distance <= 1.0
        {
            if pd.send_bang_to("dug_object").is_err() {
                warn_once!("pd: no reciever named 'dug_object'");
            }

            let start_time = self.clock.time;
            let start_altitude = -0.5;

            let behaviour = move |this: &mut Node, clock: &Clock| {
                let total_time: f32 = (clock.time - start_time).cast_approx();

                let delete_time = 4.0;
                if total_time > delete_time {
                    this.to_delete = true;
                }

                let Object::Mesh(mesh) = this.find(|obj| matches!(obj, Object::Mesh(_))).unwrap()
                else {
                    unreachable!()
                };

                let (a, b) = (2.0, 2.0);
                let alpha = (if total_time < a {
                    1.0
                } else if total_time - a < b {
                    ((std::f32::consts::FRAC_PI_2 / b) * (total_time - a)).cos()
                } else {
                    0.0
                } * f32::from(u16::MAX))
                .cast_nearest();

                mesh.alpha
                    .store(alpha, std::sync::atomic::Ordering::Relaxed);

                let (a, b, c) = (4.0, 0.5, 1.0);

                let rotation = a * (total_time / b + 1.0).ln() + c;

                let altitude = 2.0 * (-(total_time + 1.0).powi(2).recip() + 1.0);

                let t = this.transform.translation;
                let translation = Vec3::new(t.x, start_altitude + altitude, t.z);
                this.set_transform(Isometry3::new(
                    translation,
                    Rotor3::from_rotation_xz(rotation).normalized(),
                ));
            };
            self.scene.push(
                Node::new(Isometry3::new(
                    Vec3::new(object.location.x, 0.0, object.location.y)
                        + 2.0
                            * Vec3::unit_x().rotated_by(Rotor3::from_rotation_xz(
                                input.camera_rotation.x + std::f32::consts::FRAC_PI_2,
                            )),
                    Rotor3::identity(),
                ))
                .mesh(self.metal_detector_objects.remove(idx).mesh)
                .behaviour(Arc::new(behaviour)),
            );
        }

        if pd.send_float_to("badness", 0.3 * badness).is_err() {
            warn_once!("pd: no reciever named 'badness'");
        }

        if pd.send_float_to("distance", 0.3 * distance).is_err() {
            warn_once!("pd: no reciever named 'distance'");
        }

        let mut behaviours: Vec<(usize, Arc<Behaviour>)> = Vec::new();
        let mut to_delete: Vec<usize> = Vec::new();

        for (idx, node) in self.scene.iter().enumerate() {
            if node.to_delete {
                to_delete.push(idx);
            }

            for object in &node.objects {
                if let Object::Behaviour(behaviour) = object {
                    behaviours.push((idx, behaviour.clone()));
                }
            }
        }

        for (node_idx, behaviour) in behaviours {
            behaviour(&mut self.scene[node_idx], &self.clock);
        }

        for node_idx in to_delete {
            self.scene.remove(node_idx);
        }
    }

    pub fn update(
        &mut self,
        input: &mut SharedState,
        pd: &mut Pd,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) {
        self.clock.update();

        self.gui.update_input(&self.clock, events, modifiers);

        let game_state = input.game_state();

        match game_state {
            GameState::Menu => (),
            GameState::Playing => self.update_playing(pd, input),
        }

        self.audio.process_events(pd, &mut input.audio_events);
    }
}

struct MetalDetectorManifest<'a> {
    location: Vec2,
    badness: f32,
    scale: f32,
    model: GltfFile<'a>,
}

impl MetalDetectorManifest<'_> {
    fn into_metal_detector_object(
        self,
        renderer: &Renderer,
        cmd_buf: &mut OneTimeSubmitCommandBuffer,
    ) -> MetalDetectorObject {
        MetalDetectorObject {
            location: self.location,
            badness: self.badness,
            mesh: load_gltf(renderer, cmd_buf, self.model, self.scale).into(),
        }
    }
}

const METAL_DETECTOR_MANIFESTS: [MetalDetectorManifest<'static>; 5] = [
    MetalDetectorManifest {
        location: Vec2::new(8.0, -8.0),
        badness: 0.0,
        scale: 1.0,
        model: GltfFile::Bytes(include_bytes!("../test-objects/tetrahedron.glb")),
    },
    MetalDetectorManifest {
        location: Vec2::new(15.0, -6.0),
        badness: 0.35,
        scale: 1.0,
        model: GltfFile::Bytes(include_bytes!("../test-objects/cube.glb")),
    },
    MetalDetectorManifest {
        location: Vec2::new(-12.0, 9.0),
        badness: 0.5,
        scale: 1.0,
        model: GltfFile::Bytes(include_bytes!("../test-objects/octahedron.glb")),
    },
    MetalDetectorManifest {
        location: Vec2::new(-16.0, -16.0),
        badness: 0.7,
        scale: 1.0,
        model: GltfFile::Bytes(include_bytes!("../test-objects/dodecahedron.glb")),
    },
    MetalDetectorManifest {
        location: Vec2::new(20.0, 17.0),
        badness: 1.0,
        scale: 1.0,
        model: GltfFile::Bytes(include_bytes!("../test-objects/icosahedron.glb")),
    },
];

struct MetalDetectorObject {
    location: Vec2,
    badness: f32,
    mesh: Arc<Mesh>,
}
