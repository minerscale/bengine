#![allow(clippy::too_many_lines)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::if_not_else)]
#![windows_subsystem = "windows"]

mod event_loop;
mod node;
mod player;
mod renderer;
mod shader_pipelines;
mod vertex;
mod mesh;

use std::{io::Cursor, mem::offset_of, ptr::addr_of, rc::Rc};

use obj::raw::RawObj;
use player::get_movement_impulse;
use renderer::{
    HEIGHT, WIDTH,
    buffer::MappedBuffer,
    command_buffer::ActiveMultipleSubmitCommandBuffer,
    device::Device,
    image::{Image, SwapchainImage},
    pipeline::VertexPushConstants,
    render_pass::RenderPass,
    sampler::Sampler,
    texture::Texture,
};

use mesh::Mesh;

use ash::vk;
use event_loop::EventLoop;
use log::info;
use node::{GameTree, Node, Object};
use rapier3d::{
    self,
    math::AngVector,
    na::{self, vector},
    prelude::{
        CCDSolver, ColliderBuilder, ColliderSet, ColliderShape, DefaultBroadPhase, ImpulseJointSet,
        IntegrationParameters, IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline,
        QueryPipeline, RigidBodyBuilder, RigidBodySet,
    },
};
use renderer::{Renderer, UniformBufferObject};
use ultraviolet::{Isometry3, Rotor3, Vec2, Vec3};

use sdl3::event::Event;

fn main() {
    env_logger::init();
    let mut gfx = Renderer::new(WIDTH, HEIGHT, &shader_pipelines::PIPELINES);

    let cube_2_scale = Vec3::new(1.0, 0.4, 1.0);

    let (/*teapot, suzanne,*/ room, cube_1, cube_2 /*, icosehedron*/) = gfx
        .command_pool
        .one_time_submit(gfx.device.graphics_queue, |cmd_buf| {
            macro_rules! image {
                ($filename:literal) => {
                    Rc::new(Image::from_bytes(
                        &gfx.instance,
                        gfx.device.physical_device,
                        &gfx.device.device,
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
                gfx.device.physical_device,
            ));

            let grid = texture!(sampler, image!("../textures/grid.png"));

            let middle_grey = texture!(sampler, image!("../test-scene/middle-grey.png"));

            (
                Object::Model((mesh!("../test-objects/ground-plane.obj", None), grid)),
                Object::Model((
                    mesh!("../test-scene/icosehedron.obj", None),
                    middle_grey.clone(),
                )),
                Object::Model((
                    mesh!("../test-scene/cube.obj", Some(cube_2_scale)),
                    middle_grey,
                )),
            )
        });

    fn collider_from_obj(
        mesh: &RawObj,
        scale: Option<Vec3>,
        transform: Option<Vec3>,
    ) -> ColliderShape {
        type Point = na::Point<f32, 3>;

        let vertices: Box<[Point]> = mesh
            .positions
            .iter()
            .map(|v| {
                Point::from_slice(
                    transform
                        .unwrap_or_else(|| {
                            Vec3::zero()
                                + scale.unwrap_or_else(Vec3::one) * Vec3::new(v.0, v.1, v.2)
                        })
                        .as_array(),
                )
            })
            .collect();

        ColliderShape::convex_hull(&vertices).unwrap()
    }

    let mut rigid_body_set = RigidBodySet::new();
    let mut collider_set = ColliderSet::new();

    // Make the floor
    collider_set.insert(
        ColliderBuilder::cuboid(100.0, 0.1, 100.0)
            .translation(vector![0.0, -0.1, 0.0])
            .build(),
    );

    // Create the boxes
    let cube_1_collider = ColliderBuilder::new(collider_from_obj(
        &obj::raw::parse_obj(&include_bytes!("../test-scene/icosehedron.obj")[..]).unwrap(),
        None,
        None,
    ))
    .build();

    let cube_1_rigid_body = RigidBodyBuilder::dynamic()
        .translation(vector![3.0, 10.0, 0.0])
        .rotation(AngVector::new(0.5, 1.2, 3.1));
    let cube_1_handle = rigid_body_set.insert(cube_1_rigid_body);
    collider_set.insert_with_parent(cube_1_collider, cube_1_handle, &mut rigid_body_set);

    let cube_2_collider = ColliderBuilder::new(collider_from_obj(
        &obj::raw::parse_obj(&include_bytes!("../test-scene/cube.obj")[..]).unwrap(),
        Some(cube_2_scale),
        None,
    ))
    .build();

    let cube_2_rigid_body = RigidBodyBuilder::dynamic().translation(vector![0.0, 5.0, 0.0]);
    let cube_2_handle = rigid_body_set.insert(cube_2_rigid_body);
    collider_set.insert_with_parent(cube_2_collider, cube_2_handle, &mut rigid_body_set);

    let player = RigidBodyBuilder::dynamic()
        .translation(vector![7.0, 8.0, 0.0])
        .lock_rotations();
    let player_collider = ColliderBuilder::capsule_y(0.9, 0.4)
        .restitution(0.0)
        .friction(0.0)
        .friction_combine_rule(rapier3d::prelude::CoefficientCombineRule::Multiply);

    let player_handle = rigid_body_set.insert(player);
    let player_collider_handle =
        collider_set.insert_with_parent(player_collider, player_handle, &mut rigid_body_set);

    /* Create other structures necessary for the simulation. */
    let gravity = vector![0.0, -9.81, 0.0];
    let mut integration_parameters = IntegrationParameters::default();
    let mut physics_pipeline = PhysicsPipeline::new();
    let mut island_manager = IslandManager::new();
    let mut broad_phase = DefaultBroadPhase::new();
    let mut narrow_phase = NarrowPhase::new();
    let mut impulse_joint_set = ImpulseJointSet::new();
    let mut multibody_joint_set = MultibodyJointSet::new();
    let mut ccd_solver = CCDSolver::new();
    let mut query_pipeline = QueryPipeline::new();
    let physics_hooks = ();
    let event_handler = ();

    let root_node = GameTree::new(
        Node::empty()
            .add_child(
                Node::empty() /*.add_object(teapot)*/
                    .add_object(cube_1)
                    .into(),
            )
            .add_child(
                Node::empty()
                    //.add_object(suzanne)
                    .add_object(cube_2)
                    .into(),
            )
            .add_child(Node::empty().add_object(room).into())
            .into(),
    );

    let mut event_loop = EventLoop::new(gfx.sdl_context.event_pump().unwrap());

    gfx.sdl_context
        .mouse()
        .set_relative_mouse_mode(&gfx.window, true);

    info!("finished loading");

    let start_time = std::time::Instant::now();

    let mut previous_time = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs_f64(1.0 / 60.0))
        .unwrap();

    let mut time_since_left_ground = f32::MAX;
    let mut jump_buffer = false;
    let mut previous_jump_input = false;

    event_loop.run(
        |inputs| {
            // Delta time calculation
            let new_time = std::time::Instant::now();
            let dt = (new_time - previous_time).as_secs_f32();
            let time = (new_time - start_time).as_secs_f32();

            previous_time = new_time;

            integration_parameters.set_inv_dt(1.0 / dt);

            physics_pipeline.step(
                &gravity,
                &integration_parameters,
                &mut island_manager,
                &mut broad_phase,
                &mut narrow_phase,
                &mut rigid_body_set,
                &mut collider_set,
                &mut impulse_joint_set,
                &mut multibody_joint_set,
                &mut ccd_solver,
                Some(&mut query_pipeline),
                &physics_hooks,
                &event_handler,
            );

            fn get_camera_rotor(camera_rotation: Vec2) -> Rotor3 {
                Rotor3::from_rotation_xz(camera_rotation.x)
                    * Rotor3::from_rotation_yz(camera_rotation.y)
            }

            let camera_rotation = get_camera_rotor(inputs.camera_rotation);

            let player_info = &rigid_body_set[player_handle];

            fn from_nalgebra(p: &rapier3d::na::Isometry3<f32>) -> Isometry3 {
                Isometry3::new(
                    Vec3::from(p.translation.vector.as_slice().first_chunk().unwrap()),
                    Rotor3::from_quaternion_array(
                        *p.rotation.coords.as_slice().first_chunk().unwrap(),
                    ),
                )
            }

            let player_transform = from_nalgebra(rigid_body_set[player_handle].position());

            if inputs.up && !previous_jump_input {
                jump_buffer = true;
            } else if !inputs.up && previous_jump_input {
                jump_buffer = false;
            }

            previous_jump_input = inputs.up;

            let (player_impulse, friction) = get_movement_impulse(
                &narrow_phase,
                player_collider_handle,
                inputs,
                player_info,
                camera_rotation,
                dt,
                &mut time_since_left_ground,
                &mut jump_buffer,
            );

            collider_set[player_collider_handle].set_friction(friction);

            rigid_body_set[player_handle].apply_impulse(player_impulse, true);

            let camera_transform = Isometry3::new(
                player_transform.translation + Vec3::new(0.0, 0.8, 0.0),
                camera_rotation.reversed(),
            );

            let ubo = UniformBufferObject{
                view_transform: camera_transform,
                time,
            };

            let n = root_node.root_node.borrow();

            n.children[0].borrow_mut().transform =
                from_nalgebra(rigid_body_set[cube_1_handle].position());
            n.children[1].borrow_mut().transform =
                from_nalgebra(rigid_body_set[cube_2_handle].position());

            inputs.recreate_swapchain = gfx.draw(
                |device, render_pass, command_buffer, uniform_buffer, image| {
                    record_command_buffer(
                        device,
                        render_pass,
                        command_buffer,
                        uniform_buffer,
                        image,
                        &root_node,
                        ubo,
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
                    let mut rotation = inputs.camera_rotation + Vec2::new(xrel, yrel) * SENSITIVITY;

                    rotation.y = rotation
                        .y
                        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

                    rotation
                };
            }
            Event::Window {
                timestamp: _,
                window_id: _,
                win_event: sdl3::event::WindowEvent::PixelSizeChanged(_, _),
            } => {
                inputs.recreate_swapchain = true;
            }
            _ => (),
        },
    );

    gfx.wait_idle();
}

fn record_command_buffer(
    device: &Device,
    render_pass: &RenderPass,
    command_buffer: ActiveMultipleSubmitCommandBuffer,
    uniform_buffer: &mut MappedBuffer<UniformBufferObject>,
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

    unsafe {
        let uniform_buffer_descriptor_set = [*uniform_buffer.descriptor_set];
        let ubo_mapped = uniform_buffer.mapped_memory.first_mut().unwrap();
        *ubo_mapped = ubo;

        let cmd_buf = *command_buffer;
        device.cmd_begin_render_pass(cmd_buf, &render_pass_info, vk::SubpassContents::INLINE);

        let skybox_pipeline = &render_pass.pipelines[1];
        device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, **skybox_pipeline);
        device.cmd_bind_descriptor_sets(
            cmd_buf,
            vk::PipelineBindPoint::GRAPHICS,
            skybox_pipeline.pipeline_layout,
            0,
            &uniform_buffer_descriptor_set,
            &[],
        );
        device.cmd_draw(cmd_buf, 3, 1, 0, 0);

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
                offset_of!(VertexPushConstants, model_transform)
                    .try_into()
                    .unwrap(),
                std::slice::from_raw_parts(
                    addr_of!(modelview_transform).cast::<u8>(),
                    std::mem::size_of::<Isometry3>(),
                ),
            );

            for object in &node.borrow().objects {
                let Object::Model((mesh, texture)) = object;
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

        device.cmd_end_render_pass(cmd_buf);
    }

    command_buffer
}
