#![windows_subsystem = "windows"]
#![allow(clippy::too_many_lines)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::if_not_else)]

mod audio;
mod clock;
mod event_loop;
mod mesh;
mod node;
mod physics;
mod player;
mod renderer;
mod scene;
mod shader_pipelines;
mod skybox;
mod vertex;

use player::Player;
use tracing_mutex::stdsync::Mutex;

use std::{mem::offset_of, ptr::addr_of, sync::Arc};

use audio::{Audio, AudioParameters};
use clock::{Clock, FIXED_UPDATE_INTERVAL};
use event_loop::EventLoop;
use node::{Node, Object};
use physics::{Physics, from_nalgebra};
use renderer::{
    HEIGHT, Renderer, UniformBufferObject, WIDTH, buffer::MappedBuffer,
    command_buffer::ActiveMultipleSubmitCommandBuffer, device::Device, image::SwapchainImage,
    material::MaterialProperties, render_pass::RenderPass,
};
use scene::create_scene;
use shader_pipelines::{DESCRIPTOR_SET_LAYOUTS, PIPELINES, PushConstants};
use skybox::Skybox;

use ash::vk;
use log::info;
use ultraviolet::{Isometry3, Lerp, Rotor3, Slerp, Vec2, Vec3};

pub const FOV: f32 = 100.0;

fn main() {
    env_logger::init();

    let sdl_context = sdl3::init().unwrap();

    let window = {
        sdl_context
            .video()
            .unwrap()
            .window("bengine", WIDTH, HEIGHT)
            .vulkan()
            .position_centered()
            .resizable()
            .build()
            .unwrap()
    };

    sdl_context.mouse().set_relative_mouse_mode(&window, true);

    let mut gfx = Renderer::new(WIDTH, HEIGHT, &window, &DESCRIPTOR_SET_LAYOUTS, &PIPELINES);
    let skybox = Skybox::new(&gfx);
    let mut physics = Physics::new();
    let player = Arc::new(Mutex::new(Player::new(&mut physics)));
    let physics = Arc::new(Mutex::new(physics));

    let scene = Arc::new(Mutex::new(create_scene(&gfx, &mut physics.lock().unwrap())));

    let clock = Arc::new(Mutex::new(Clock::new()));
    let update_clock = clock.clone();
    let audio = Audio::new();

    let render_lock = Arc::new(Mutex::new(()));

    let mut event_loop = EventLoop::new(sdl_context.event_pump().unwrap());

    info!("finished loading");

    event_loop.run(
        |input| {
            let extent = gfx.swapchain.images[0].extent;

            let mut minput = input.lock().unwrap();

            let framebuffer_resized = if let Some(framebuffer_size) = minput.framebuffer_resized {
                gfx.window_size = framebuffer_size;
                true
            } else {
                false
            };

            minput.framebuffer_resized = None;

            drop(minput);

            gfx.acquire_next_image(framebuffer_resized);

            gfx.draw(
                |device, render_pass, command_buffer, uniform_buffer, image| {
                    let _render_lock = render_lock.lock().unwrap();

                    let clock = clock.lock().unwrap().clone();
                    let interpolation_factor =
                        ((std::time::Instant::now() - clock.previous_time).as_secs_f64()
                            / FIXED_UPDATE_INTERVAL) as f32;

                    let player = player.lock().unwrap();
                    let player_transform = player
                        .previous_position
                        .lerp(player.position, interpolation_factor);
                    drop(player);

                    let minput = input.lock().unwrap();
                    let camera_rotation = get_camera_rotor(
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
                        time: clock.time,
                        fov: ez,
                        scale_y: (extent.width as f32) / (extent.height as f32),
                    };

                    record_command_buffer(
                        device,
                        render_pass,
                        command_buffer,
                        uniform_buffer,
                        interpolation_factor,
                        &skybox,
                        image,
                        &scene.lock().unwrap(),
                        ubo,
                    )
                },
            );

            gfx.present();
        },
        |input| {
            let _render_lock = render_lock.lock().unwrap();

            let mut clock = update_clock.lock().unwrap();
            clock.update();
            let dt = clock.dt;
            drop(clock);

            let mut physics = physics.lock().unwrap();
            //let mut player = player.lock().unwrap();

            let mut player = player.lock().unwrap();
            let player_rigid_body_handle = player.rigid_body_handle;

            let input = input.lock().unwrap();

            player.update(
                &mut physics,
                &input,
                get_camera_rotor(input.camera_rotation),
                dt,
            );

            let player_transform =
                from_nalgebra(physics.rigid_body_set[player_rigid_body_handle].position());

            drop(input);

            physics.step(&mut scene.lock().unwrap(), &mut player, dt);

            drop(player);
            drop(physics);

            let gems_and_jewel_location = Vec2::new(8.0, 8.0);
            let distance = (Vec2::new(
                player_transform.translation.x,
                player_transform.translation.z,
            ) - gems_and_jewel_location)
                .mag();

            audio
                .parameter_stream
                .send(AudioParameters::new(distance.into()))
                .unwrap();
        },
    );

    gfx.wait_idle();
}

fn interpolate_isometry(a: Isometry3, b: Isometry3, t: f32) -> Isometry3 {
    Isometry3::new(
        a.translation.lerp(b.translation, t),
        a.rotation.slerp(b.rotation, t).normalized(),
    )
}

fn record_command_buffer(
    device: &Device,
    render_pass: &RenderPass,
    command_buffer: ActiveMultipleSubmitCommandBuffer,
    uniform_buffers: &mut [MappedBuffer<UniformBufferObject>],
    interpolation_factor: f32,
    skybox: &Skybox,
    image: &SwapchainImage,
    scene: &[Node],
    ubo: UniformBufferObject,
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

    let uniform_buffer = &mut uniform_buffers[0];

    let uniform_buffer_descriptor_set = [*uniform_buffer.descriptor_set];
    let ubo_mapped = uniform_buffer.mapped_memory.first_mut().unwrap();
    *ubo_mapped = ubo;

    let cmd_buf = *command_buffer;

    let command_buffer = skybox.render(device, command_buffer, &uniform_buffer.descriptor_set);

    unsafe {
        device.cmd_begin_render_pass(cmd_buf, &render_pass_info, vk::SubpassContents::INLINE);

        let command_buffer = skybox.blit(
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

        for node in scene {
            for object in &node.objects {
                if let Object::Mesh(mesh) = object {
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

                    device.cmd_push_constants(
                        cmd_buf,
                        pipeline.pipeline_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        offset_of!(PushConstants, model_transform)
                            .try_into()
                            .unwrap(),
                        std::slice::from_raw_parts(
                            addr_of!(modelview_transform).cast::<u8>(),
                            std::mem::size_of::<Isometry3>(),
                        ),
                    );

                    let mesh = mesh.as_ref();

                    for primitive in mesh {
                        let descriptor_sets = [*primitive.material.descriptor_set];

                        device.cmd_push_constants(
                            cmd_buf,
                            pipeline.pipeline_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            offset_of!(PushConstants, material_properties)
                                .try_into()
                                .unwrap(),
                            std::slice::from_raw_parts(
                                addr_of!(primitive.material.properties).cast::<u8>(),
                                std::mem::size_of::<MaterialProperties>(),
                            ),
                        );

                        device.cmd_bind_descriptor_sets(
                            cmd_buf,
                            vk::PipelineBindPoint::GRAPHICS,
                            pipeline.pipeline_layout,
                            1,
                            &descriptor_sets,
                            &[],
                        );

                        let vertex_buffers = [primitive.vertex_buffer.buffer];
                        let offsets = [vk::DeviceSize::from(0u64)];

                        device.cmd_bind_vertex_buffers(cmd_buf, 0, &vertex_buffers, &offsets);
                        device.cmd_bind_index_buffer(
                            cmd_buf,
                            primitive.index_buffer.buffer,
                            0,
                            vk::IndexType::UINT32,
                        );

                        device.cmd_draw_indexed(
                            cmd_buf,
                            primitive.index_buffer.len().try_into().unwrap(),
                            1,
                            0,
                            0,
                            0,
                        );
                    }
                }
            }
        }

        device.cmd_end_render_pass(cmd_buf);

        command_buffer
    }
}

fn get_camera_rotor(camera_rotation: Vec2) -> Rotor3 {
    Rotor3::from_rotation_xz(camera_rotation.x) * Rotor3::from_rotation_yz(camera_rotation.y)
}
