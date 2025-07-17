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

use std::{
    mem::offset_of,
    ptr::addr_of,
    sync::{Arc, Mutex},
};

use audio::{Audio, AudioParameters};
use clock::Clock;
use event_loop::EventLoop;
use node::{GameTree, Object};
use physics::Physics;
use player::Player;
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
use ultraviolet::{Isometry3, Rotor3, Vec2, Vec3};

pub const FOV: f32 = 100.0;

fn main() {
    env_logger::init();

    let mut gfx = Renderer::new(WIDTH, HEIGHT, &DESCRIPTOR_SET_LAYOUTS, &PIPELINES);
    let skybox = Skybox::new(&gfx);
    let physics = Arc::new(Mutex::new(Physics::new()));
    let root_node = create_scene(&gfx, &mut physics.lock().unwrap());
    let player = Arc::new(Mutex::new(Player::new(&mut physics.lock().unwrap())));
    let clock = Arc::new(Mutex::new(Clock::new()));
    let update_clock = clock.clone();
    let audio = Audio::new();

    let mut event_loop = EventLoop::new(gfx.sdl_context.event_pump().unwrap());

    info!("finished loading");

    event_loop.run(
        |input| {
            let player_transform = {
                let physics = physics.lock().unwrap();

                for (_transform, node) in root_node.breadth_first() {
                    let transform = node.borrow().objects.iter().find_map(|o| {
                        if let Object::RigidBody((_, rigid_body_handle)) = o {
                            Some(from_nalgebra(
                                physics.rigid_body_set[*rigid_body_handle].position(),
                            ))
                        } else {
                            None
                        }
                    });

                    if let Some(transform) = transform {
                        node.borrow_mut().transform = transform;
                    }
                }

                from_nalgebra(
                    physics.rigid_body_set[player.lock().unwrap().rigid_body_handle].position(),
                )
            };

            let minput = input.lock().unwrap();
            let recreate_swapchain = minput.recreate_swapchain;
            let camera_rotation = get_camera_rotor(minput.camera_rotation);
            drop(minput);

            let camera_transform = Isometry3::new(
                player_transform.translation + Vec3::new(0.0, 0.8, 0.0),
                camera_rotation.reversed(),
            );

            let fov = FOV.to_radians();
            let ez = f32::tan(fov / 2.0).recip();
            let extent = gfx.swapchain.images[0].extent;
            let ubo = UniformBufferObject {
                view_transform: camera_transform,
                time: clock.lock().unwrap().time,
                fov: ez,
                scale_y: (extent.width as f32) / (extent.height as f32),
            };

            let recreate_swapchain = gfx.draw(
                |device, render_pass, command_buffer, uniform_buffer, image| {
                    record_command_buffer(
                        device,
                        render_pass,
                        command_buffer,
                        uniform_buffer,
                        &skybox,
                        image,
                        &root_node,
                        ubo,
                    )
                },
                recreate_swapchain,
            );

            input.lock().unwrap().recreate_swapchain = recreate_swapchain;
        },
        |input| {
            let mut physics = physics.lock().unwrap();
            let mut player = player.lock().unwrap();
            let mut clock = update_clock.lock().unwrap();
            clock.update();
            physics.step(clock.dt);

            let input = input.lock().unwrap();

            player.update(
                &mut physics,
                &input,
                get_camera_rotor(input.camera_rotation),
                clock.dt,
            );
            drop(input);
            drop(clock);

            let player_transform =
                from_nalgebra(physics.rigid_body_set[player.rigid_body_handle].position());
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

fn record_command_buffer(
    device: &Device,
    render_pass: &RenderPass,
    command_buffer: ActiveMultipleSubmitCommandBuffer,
    uniform_buffers: &mut [MappedBuffer<UniformBufferObject>],
    skybox: &Skybox,
    image: &SwapchainImage,
    root_node: &GameTree,
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

        for (transform, node) in root_node.breadth_first() {
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

            for object in &node.borrow().objects {
                if let Object::Mesh(mesh) = object {
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

fn from_nalgebra(p: &rapier3d::na::Isometry3<f32>) -> Isometry3 {
    Isometry3::new(
        Vec3::from(p.translation.vector.as_slice().first_chunk().unwrap()),
        Rotor3::from_quaternion_array(*p.rotation.coords.as_slice().first_chunk().unwrap()),
    )
}
