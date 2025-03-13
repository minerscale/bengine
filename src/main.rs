#![windows_subsystem = "windows"]

pub mod buffer;
pub mod command_buffer;
pub mod debug_messenger;
pub mod descriptors;
pub mod device;
pub mod event_loop;
pub mod image;
pub mod instance;
pub mod mesh;
pub mod node;
pub mod pipeline;
pub mod render_pass;
pub mod renderer;
pub mod sampler;
pub mod shader_module;
pub mod surface;
pub mod swapchain;
pub mod synchronization;
pub mod texture;
pub mod vertex;

use std::{io::Cursor, mem::offset_of, ptr::addr_of, rc::Rc};

use ash::vk;
use buffer::MappedBuffer;
use command_buffer::ActiveMultipleSubmitCommandBuffer;

use device::Device;
use event_loop::EventLoop;
use image::{Image, SwapchainImage};
use log::info;
use mesh::Mesh;
use node::{Node, Object};
use pipeline::Pipeline;
use renderer::{Renderer, UniformBufferObject};
use sampler::Sampler;

use texture::Texture;
use ultraviolet::{Isometry3, Rotor3, Vec2, Vec3};

use sdl2::event::Event;
use vertex::Vertex;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

#[repr(C)]
pub struct VertexPushConstants {
    model_transform: Isometry3,
}

fn main() {
    env_logger::init();
    let mut gfx = Renderer::new(WIDTH, HEIGHT);

    let (teapot, suzanne, room) =
        gfx.command_pool
            .one_time_submit(gfx.device.graphics_queue, |cmd_buf| {
                macro_rules! image {
                    ($filename:literal) => {
                        Rc::new(Image::from_bytes(
                            &gfx.instance,
                            gfx.device.physical_device,
                            gfx.device.device.clone(),
                            cmd_buf,
                            include_bytes!($filename),
                        ))
                    };
                }

                macro_rules! mesh {
                    ($filename:literal) => {
                        Rc::new(Mesh::new(
                            &gfx.instance,
                            gfx.device.physical_device,
                            gfx.device.device.clone(),
                            Cursor::new(include_bytes!($filename)),
                            cmd_buf,
                        ))
                    };
                }

                macro_rules! texture {
                    ($sampler:expr, $texture:expr) => {
                        Rc::new(Texture::new(
                            &gfx.device,
                            $texture.clone(),
                            $sampler.clone(),
                            &gfx.descriptor_pool,
                            &gfx.texture_layout,
                        ))
                    };
                }

                let sampler = Rc::new(Sampler::new(
                    &gfx.instance,
                    gfx.device.device.clone(),
                    &gfx.device.physical_device,
                ));

                let agad_texture = texture!(sampler, image!("../textures/agadwheel.png"));

                let floor_tiles = texture!(
                    sampler,
                    image!("../test-scene/textures/floor_tiles_06_diff_1k.jpg")
                );

                (
                    Object::Model((
                        mesh!("../test-objects/teapot-triangulated.obj"),
                        agad_texture.clone(),
                    )),
                    Object::Model((mesh!("../test-objects/suzanne.obj"), agad_texture)),
                    Object::Model((mesh!("../test-scene/room.obj"), floor_tiles)),
                )
            });

    let mut root_node = Node::empty()
        .add_child(Node::empty().add_object(teapot))
        .add_child(Node::empty().add_child(Node::empty().add_object(suzanne)))
        .add_child(Node::empty().add_object(room));

    let mut event_loop = EventLoop::new(gfx.sdl_context.event_pump().unwrap());

    let mut camera_position = Vec3::new(6.0, 5.0, 6.0);

    fn get_camera_rotor(camera_rotation: Vec2) -> Rotor3 {
        Rotor3::from_rotation_xz(camera_rotation.x) * Rotor3::from_rotation_yz(camera_rotation.y)
    }

    gfx.sdl_context.mouse().set_relative_mouse_mode(true);

    info!("finished loading");

    let start_time = std::time::Instant::now();

    let mut previous_time =
        std::time::Instant::now() - std::time::Duration::from_secs_f64(1.0 / 60.0);
    event_loop.run(
        |inputs| {
            // Delta time calculation
            let new_time = std::time::Instant::now();
            let dt = (new_time - previous_time).as_secs_f32();
            previous_time = new_time;

            let time_secs = (new_time - start_time).as_secs_f32();

            let camera_rotation = get_camera_rotor(inputs.camera_rotation);

            root_node.children[0].transform = Isometry3::new(
                Vec3::new(0.0, 0.0, 0.0),
                Rotor3::from_rotation_xz(1.0 * time_secs),
            );

            root_node.children[1].children[0].transform = Isometry3::new(
                Vec3::new(5.0, 2.0, 0.0),
                Rotor3::from_rotation_xz(3.0 * time_secs),
            );

            root_node.children[1].transform = Isometry3::new(
                Vec3::new(0.0, 0.0, 0.0),
                Rotor3::from_rotation_xz(0.5 * time_secs),
            );

            const MOVEMENT_SPEED: f32 = 5.0;
            let camera_movement = if inputs.forward {
                Vec3::unit_z()
            } else if inputs.backward {
                -Vec3::unit_z()
            } else {
                Vec3::zero()
            } + if inputs.left {
                Vec3::unit_x()
            } else if inputs.right {
                -Vec3::unit_x()
            } else {
                Vec3::zero()
            };

            let vertical_movement = if inputs.up {
                Vec3::unit_y()
            } else if inputs.down {
                -Vec3::unit_y()
            } else {
                Vec3::zero()
            };

            camera_position += (vertical_movement + camera_movement.rotated_by(camera_rotation))
                * (MOVEMENT_SPEED * dt);

            let camera_transform = Isometry3::new(camera_position, camera_rotation.reversed());

            inputs.recreate_swapchain = gfx.draw(
                |device, pipeline, command_buffer, uniform_buffer, image| {
                    record_command_buffer(
                        device,
                        pipeline,
                        command_buffer,
                        uniform_buffer,
                        image,
                        &root_node,
                        camera_transform,
                    )
                },
                inputs.recreate_swapchain,
            );
        },
        |event, inputs| match event {
            Event::Quit { timestamp: _ } => inputs.quit = true,
            Event::KeyDown {
                keycode: Some(key),
                repeat: false,
                ..
            } => inputs.set_input(key, true),
            Event::KeyUp {
                keycode: Some(key),
                repeat: false,
                ..
            } => inputs.set_input(key, false),
            Event::MouseMotion {
                timestamp: _,
                window_id: _,
                which: _,
                mousestate: _,
                x: _,
                y: _,
                xrel,
                yrel,
            } => {
                const SENSITIVITY: f32 = 0.005;

                inputs.camera_rotation = {
                    let mut rotation =
                        inputs.camera_rotation + Vec2::new(xrel as f32, yrel as f32) * SENSITIVITY;

                    rotation.y = rotation
                        .y
                        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

                    rotation
                };
            }
            Event::Window {
                timestamp: _,
                window_id: _,
                win_event: sdl2::event::WindowEvent::SizeChanged(_, _),
            } => {
                inputs.recreate_swapchain = true;
            }
            _ => (),
        },
    );

    gfx.wait_idle();
}

pub fn record_command_buffer(
    device: &Device,
    pipeline: &Pipeline,
    command_buffer: ActiveMultipleSubmitCommandBuffer,
    uniform_buffer: &mut MappedBuffer<UniformBufferObject>,
    image: &SwapchainImage,
    root_node: &Node,
    camera_transform: Isometry3,
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
        .render_pass(*pipeline.render_pass)
        .framebuffer(image.framebuffer)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: image.extent,
        })
        .clear_values(&clear_color);

    let viewport = [vk::Viewport::default()
        .x(0.0)
        .y(0.0)
        .width(image.extent.width as f32)
        .height(image.extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0)];

    unsafe {
        let cmd_buf = *command_buffer;
        device.cmd_begin_render_pass(cmd_buf, &render_pass_info, vk::SubpassContents::INLINE);

        device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, **pipeline);

        device.cmd_set_viewport(cmd_buf, 0, &viewport);

        let scissor = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D {
                width: image.extent.width,
                height: image.extent.height,
            },
        }];

        device.cmd_set_scissor(cmd_buf, 0, &scissor);

        let ubo = uniform_buffer.mapped_memory.first_mut().unwrap();

        *ubo = UniformBufferObject {
            view_transform: camera_transform,
        };

        let uniform_buffer_descriptor_set = [*uniform_buffer.descriptor_set];
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
                translation: (transform.translation - camera_transform.translation).rotated_by(camera_transform.rotation),
                rotation: camera_transform.rotation * transform.rotation,
            };

            //let mut modelview_transform = transform.clone();

            //modelview_transform.append_isometry(camera_transform);

            device.cmd_push_constants(
                cmd_buf,
                pipeline.pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                offset_of!(VertexPushConstants, model_transform) as u32,
                std::slice::from_raw_parts(
                    addr_of!(modelview_transform) as *const u8,
                    std::mem::size_of::<Isometry3>(),
                ),
            );

            for object in &node.objects {
                match object {
                    Object::Model((mesh, texture)) => {
                        let descriptor_sets = [texture.descriptor_set.descriptor_set];

                        device.cmd_bind_descriptor_sets(
                            cmd_buf,
                            vk::PipelineBindPoint::GRAPHICS,
                            pipeline.pipeline_layout,
                            1,
                            &descriptor_sets,
                            &[],
                        );

                        let mesh = mesh.as_ref();

                        let vertex_buffers = [mesh.vertex_buffer.buffer];
                        let offsets = [vk::DeviceSize::from(0u64)];

                        device.cmd_bind_vertex_buffers(cmd_buf, 0, &vertex_buffers, &offsets);
                        device.cmd_bind_index_buffer(
                            cmd_buf,
                            mesh.index_buffer.buffer,
                            0,
                            vk::IndexType::UINT32,
                        );

                        device.cmd_draw_indexed(
                            cmd_buf,
                            mesh.index_buffer.len().try_into().unwrap(),
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
    }

    command_buffer
}
