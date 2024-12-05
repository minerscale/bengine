pub mod buffer;
pub mod command_buffer;
pub mod debug_messenger;
pub mod device;
pub mod event_loop;
pub mod image;
pub mod instance;
pub mod pipeline;
pub mod render_pass;
pub mod renderer;
pub mod shader_module;
pub mod surface;
pub mod swapchain;
pub mod synchronization;
pub mod vertex;

use core::f32;
use std::{
    cell::{Cell, RefCell},
    io::Cursor,
    mem::offset_of,
    ptr::addr_of,
};

use ash::vk;
use buffer::{Buffer, StagedBuffer};
use command_buffer::ActiveMultipleSubmitCommandBuffer;

use device::Device;
use event_loop::EventLoop;
use image::SwapchainImage;
use obj::{load_obj, Obj};
use pipeline::Pipeline;
use renderer::Renderer;

use geometric_algebra::{bivector::BiVector, rotor::Rotor, vec2::Vec2, vector::Vector};
use sdl2::{event::Event, keyboard::Keycode};
use vertex::Vertex;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

#[repr(C, align(32))]
pub struct VertexPushConstants {
    camera_rotation: Rotor<f32>,
    camera_position: Vector<f32>,
}

#[repr(C, align(16))]
pub struct FragmentPushConstants {
    camera_vector: Vector<f32>,
}

#[repr(C)]
pub struct PushConstants {
    vertex: VertexPushConstants,
    fragment: FragmentPushConstants,
}

#[derive(Debug, Default)]
struct Inputs {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
}

impl Inputs {
    fn set_input(&mut self, key: sdl2::keyboard::Keycode, pressed: bool) {
        type K = Keycode;
        match key {
            K::W => self.forward = pressed,
            K::R => self.backward = pressed,
            K::A => self.left = pressed,
            K::S => self.right = pressed,
            K::SPACE => self.up = pressed,
            K::C => self.down = pressed,
            _ => (),
        }
    }
}

fn main() {
    env_logger::init();
    let mut gfx = Renderer::new(WIDTH, HEIGHT);


    let teapot: Obj<Vertex, u32> = load_obj(Cursor::new(include_bytes!("../newell_teaset/teapot-triangulated.obj")))
    .unwrap();

    let staging_command_buffer = gfx.command_pool.create_one_time_submit_command_buffer();

    let vertex_buffer = StagedBuffer::new(
        &gfx.instance,
        gfx.device.device.clone(),
        gfx.device.physical_device,
        &staging_command_buffer,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        &teapot.vertices,
    );

    let index_buffer = StagedBuffer::new(
        &gfx.instance,
        gfx.device.device.clone(),
        gfx.device.physical_device,
        &staging_command_buffer,
        vk::BufferUsageFlags::INDEX_BUFFER,
        &teapot.indices,
    );

    staging_command_buffer.submit(gfx.device.graphics_queue, &gfx.command_pool);

    let mut event_loop = EventLoop::new(gfx.sdl_context.event_pump().unwrap());

    let framebuffer_resized = Cell::new(false);

    let camera_rotation = Cell::new(Vec2::<f32>::new(f32::consts::FRAC_PI_2, 0.0));
    let mut camera_position = Vector::<f32>::new(10.0, 6.0, 0.0);

    fn get_camera_rotor(camera_rotation: Vec2<f32>) -> Rotor<f32> {
        BiVector::<f32>::E23.rotor(camera_rotation.e2)
            * BiVector::<f32>::E31.rotor(camera_rotation.e1)
    }

    let inputs = RefCell::new(Inputs::default());

    gfx.sdl_context.mouse().set_relative_mouse_mode(true);

    let mut previous_time =
        std::time::Instant::now() - std::time::Duration::from_secs_f64(1.0 / 60.0);
    event_loop.run(
        || {
            // Delta time calculation
            let new_time = std::time::Instant::now();
            let dt = (new_time - previous_time).as_secs_f32();
            previous_time = new_time;

            let camera_rotation = get_camera_rotor(camera_rotation.get());

            let inputs = inputs.borrow();

            const MOVEMENT_SPEED: f32 = 5.0;
            let camera_movement = if inputs.forward {
                -Vector::E3
            } else if inputs.backward {
                Vector::E3
            } else {
                Vector::ZERO
            } + if inputs.left {
                Vector::E1
            } else if inputs.right {
                -Vector::E1
            } else {
                Vector::ZERO
            };

            let vertical_movement = if inputs.up {
                Vector::E2
            } else if inputs.down {
                -Vector::E2
            } else {
                Vector::ZERO
            };

            camera_position = camera_position
                + (vertical_movement + camera_movement.rotate(camera_rotation.conjugate()))
                    .scalar_product(MOVEMENT_SPEED * dt);

            let push_constants = PushConstants {
                vertex: VertexPushConstants {
                    camera_position,
                    camera_rotation,
                },
                fragment: FragmentPushConstants {
                    camera_vector: Vector::<f32>::new(1.0, 1.0, 1.0)
                        .norm()
                        .rotate(camera_rotation.conjugate()),
                },
            };

            gfx.draw(
                |device, pipeline, command_buffer, image| {
                    record_command_buffer(
                        device,
                        pipeline,
                        command_buffer,
                        image,
                        &vertex_buffer,
                        &index_buffer,
                        &push_constants,
                    )
                },
                &framebuffer_resized,
            );
        },
        |event| match event {
            Event::Quit { timestamp: _ } => true,
            Event::KeyDown {
                keycode: Some(Keycode::ESCAPE),
                ..
            } => true,
            Event::KeyDown {
                keycode: Some(key),
                repeat: false,
                ..
            } => {
                inputs.borrow_mut().set_input(key, true);
                false
            }
            Event::KeyUp {
                keycode: Some(key),
                repeat: false,
                ..
            } => {
                inputs.borrow_mut().set_input(key, false);
                false
            }
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

                camera_rotation.set({
                    let mut rotation = camera_rotation.get()
                        + Vec2::<f32> {
                            e1: xrel as f32,
                            e2: -yrel as f32,
                        }
                        .scalar_product(SENSITIVITY);

                    rotation.e2 = rotation
                        .e2
                        .clamp(-f32::consts::FRAC_PI_2, f32::consts::FRAC_PI_2);

                    rotation
                });

                false
            }
            Event::Window {
                timestamp: _,
                window_id: _,
                win_event: sdl2::event::WindowEvent::SizeChanged(_, _),
            } => {
                framebuffer_resized.set(true);
                false
            }
            _ => false,
        },
    );

    gfx.wait_idle();
}

pub fn record_command_buffer(
    device: &Device,
    pipeline: &Pipeline,
    command_buffer: ActiveMultipleSubmitCommandBuffer,
    image: &SwapchainImage,
    vertex_buffer: &Buffer<Vertex>,
    index_buffer: &Buffer<u32>,
    push_constants: &PushConstants,
) -> ActiveMultipleSubmitCommandBuffer {
    unsafe {
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

        let cmd_buf = *command_buffer;

        device.cmd_begin_render_pass(cmd_buf, &render_pass_info, vk::SubpassContents::INLINE);

        device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, **pipeline);

        let viewport = [vk::Viewport::default()
            .x(0.0)
            .y(0.0)
            .width(image.extent.width as f32)
            .height(image.extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0)];

        device.cmd_set_viewport(cmd_buf, 0, &viewport);

        let scissor = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D {
                width: image.extent.width,
                height: image.extent.height,
            },
        }];

        device.cmd_set_scissor(cmd_buf, 0, &scissor);

        let vertex_buffers = [vertex_buffer.buffer];
        let offsets = [vk::DeviceSize::from(0u64)];

        device.cmd_bind_vertex_buffers(cmd_buf, 0, &vertex_buffers, &offsets);
        device.cmd_bind_index_buffer(cmd_buf, index_buffer.buffer, 0, vk::IndexType::UINT32);

        device.cmd_push_constants(
            cmd_buf,
            pipeline.pipeline_layout,
            vk::ShaderStageFlags::VERTEX,
            0,
            std::slice::from_raw_parts(
                addr_of!(push_constants.vertex) as *const u8,
                std::mem::size_of::<VertexPushConstants>(),
            ),
        );

        device.cmd_push_constants(
            cmd_buf,
            pipeline.pipeline_layout,
            vk::ShaderStageFlags::FRAGMENT,
            offset_of!(PushConstants, fragment) as u32,
            std::slice::from_raw_parts(
                addr_of!(push_constants.fragment) as *const u8,
                std::mem::size_of::<FragmentPushConstants>(),
            ),
        );
        device.cmd_draw_indexed(cmd_buf, index_buffer.len().try_into().unwrap(), 1, 0, 0, 0);

        device.cmd_end_render_pass(cmd_buf);

        command_buffer
    }
}
