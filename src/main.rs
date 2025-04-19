#![windows_subsystem = "windows"]

mod collision;
mod event_loop;
mod node;
pub mod physics;
mod renderer;

use std::{io::Cursor, mem::offset_of, ptr::addr_of, rc::Rc};

use physics::{do_physics, RigidBody};
use renderer::{
    buffer::MappedBuffer,
    command_buffer::ActiveMultipleSubmitCommandBuffer,
    device::Device,
    image::{Image, SwapchainImage},
    mesh::Mesh,
    pipeline::{Pipeline, VertexPushConstants},
    sampler::Sampler,
    texture::Texture,
    HEIGHT, WIDTH,
};

use ash::vk;
use collision::Polyhedron;
use event_loop::EventLoop;
use log::info;
use node::{GameTree, Node, Object};
use renderer::{Renderer, UniformBufferObject};
use ultraviolet::{Isometry3, Rotor3, Vec2, Vec3};

use sdl2::event::Event;

fn main() {
    env_logger::init();
    let mut gfx = Renderer::new(WIDTH, HEIGHT);

    macro_rules! collider {
        ($filename:literal, $scale:expr, $transform:expr) => {
            node::Object::Collider(Polyhedron::new(
                Cursor::new(include_bytes!($filename)),
                $scale,
                $transform,
            ))
        };
    }

    let cube_inverse_moment_of_inertia = |mass: f32, scale: Vec3| {
        12.0 / mass * {
            let (x, y, z) = scale.into();
            Vec3::new(
                1.0 / (y * y + z * z),
                1.0 / (x * x + z * z),
                1.0 / (x * x + y * y),
            )
        }
    };

    let cube_1_scale = Vec3::new(0.1, 0.5, 1.0);
    let cube_1_mass = 200.0;
    let cube_1_inverse_moment_of_inertia =
        cube_inverse_moment_of_inertia(cube_1_mass, cube_1_scale);

    let cube_2_scale = Vec3::new(1.0, 0.4, 1.0);
    let cube_2_mass = 100.0;
    let cube_2_inverse_moment_of_inertia =
        cube_inverse_moment_of_inertia(cube_2_mass, cube_2_scale);

    let (/*teapot, suzanne,*/ room, cube_1, cube_2 /*, icosehedron*/) = gfx
        .command_pool
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
                ($filename:literal, $scale:expr) => {
                    Rc::new(Mesh::new(
                        &gfx.instance,
                        gfx.device.physical_device,
                        gfx.device.device.clone(),
                        Cursor::new(include_bytes!($filename)),
                        cmd_buf,
                        $scale,
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

            //let agad_texture = texture!(sampler, image!("../textures/agadwheel.png"));

            let floor_tiles = texture!(
                sampler,
                image!("../test-scene/textures/floor_tiles_06_diff_1k.jpg")
            );

            let middle_grey = texture!(sampler, image!("../test-scene/middle-grey.png"));

            (
                /*Object::Model((
                    mesh!("../test-objects/teapot-triangulated.obj"),
                    agad_texture.clone(),
                )),
                Object::Model((mesh!("../test-objects/suzanne.obj"), agad_texture)),*/
                Object::Model((mesh!("../test-scene/room.obj", None), floor_tiles)),
                Object::Model((
                    mesh!("../test-scene/cube.obj", Some(cube_1_scale)),
                    middle_grey.clone(),
                )),
                Object::Model((
                    mesh!("../test-scene/cube.obj", Some(cube_2_scale)),
                    middle_grey.clone(),
                )),
                //Object::Model((mesh!("../test-scene/icosehedron.obj", None), middle_grey)),
            )
        });

    let root_node = GameTree::new(
        Node::empty()
            .add_child(
                Node::empty() /*.add_object(teapot)*/
                    .add_object(cube_2)
                    .add_object(collider!(
                        "../test-scene/cube.obj",
                        Some(cube_2_scale),
                        None
                    ))
                    .add_object(node::Object::RigidBody(RigidBody::new(
                        Vec3::new(0.0, 3.0, 3.0),
                        Rotor3::from_euler_angles(0.1, 0.2, 0.4),
                        Vec3::zero(),
                        Vec3::new(0.0, 0.0, 0.0),
                        cube_2_inverse_moment_of_inertia,
                        1.0 / cube_2_mass,
                    )))
                    .into(),
            )
            .add_child(
                Node::empty()
                    //.add_object(suzanne)
                    .add_object(cube_1)
                    .add_object(collider!(
                        "../test-scene/cube.obj",
                        Some(cube_1_scale),
                        None
                    ))
                    .add_object(node::Object::RigidBody(RigidBody::new(
                        Vec3::new(0.0, 7.0, 3.0),
                        Rotor3::from_euler_angles(-0.5, 0.8, 0.2),
                        -2.0 * Vec3::unit_y(),
                        Vec3::new(0.6642715, 0.20601688, -0.030171312),
                        cube_1_inverse_moment_of_inertia,
                        1.0 / cube_1_mass,
                    )))
                    .into(),
            )
            .add_child(
                Node::empty()
                    .add_object(room)
                    .add_object(collider!(
                        "../test-scene/cube.obj",
                        Some(Vec3::new(20.0, 20.0, 20.0)),
                        Some(Isometry3::new(
                            Vec3::new(0.0, -20.0, 0.0),
                            Rotor3::identity()
                        ))
                    ))
                    .add_object(node::Object::RigidBody(RigidBody::new(
                        Vec3::new(0.0, 0.0, 0.0),
                        Rotor3::identity(),
                        Vec3::zero(),
                        Vec3::new(0.0, 0.0, 0.0),
                        Vec3::zero(),
                        0.0,
                    )))
                    .into(),
            )
            .into(),
    );

    let mut event_loop = EventLoop::new(gfx.sdl_context.event_pump().unwrap());

    let mut camera_position = Vec3::new(6.0, 5.0, 6.0);

    fn get_camera_rotor(camera_rotation: Vec2) -> Rotor3 {
        Rotor3::from_rotation_xz(camera_rotation.x) * Rotor3::from_rotation_yz(camera_rotation.y)
    }

    gfx.sdl_context.mouse().set_relative_mouse_mode(true);

    info!("finished loading");

    //let start_time = std::time::Instant::now();

    let mut previous_time =
        std::time::Instant::now() - std::time::Duration::from_secs_f64(1.0 / 60.0);

    for (_, node) in root_node.breadth_first() {
        let mut new_transform = Isometry3::identity();

        for object in &mut node.borrow_mut().objects {
            match object {
                Object::RigidBody(ref mut rigid_body) => {
                    new_transform = Isometry3::new(rigid_body.position, rigid_body.orientation);
                }
                _ => (),
            }
        }

        node.borrow_mut().transform = new_transform;
    }

    event_loop.run(
        |inputs| {
            // Delta time calculation
            let new_time = std::time::Instant::now();
            let dt = 1.0 / 60.0; //(new_time - previous_time).as_secs_f32();
            previous_time = new_time;

            //let time_secs = (new_time - start_time).as_secs_f32();

            let camera_rotation = get_camera_rotor(inputs.camera_rotation);

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

            do_physics(&root_node, dt);

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
    root_node: &GameTree,
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
                translation: (transform.translation - camera_transform.translation)
                    .rotated_by(camera_transform.rotation),
                rotation: camera_transform.rotation * transform.rotation,
            };

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

            for object in node.borrow().objects.iter() {
                if let Object::Model((mesh, texture)) = object {
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

        device.cmd_end_render_pass(cmd_buf);
    }

    command_buffer
}
