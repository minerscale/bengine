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

use std::{fs::File, io::BufReader, mem::offset_of, ptr::addr_of};

use ash::vk;
use buffer::{Buffer, StagedBuffer};
use command_buffer::ActiveMultipleSubmitCommandBuffer;

use device::Device;
use event_loop::EventLoop;
use image::SwapchainImage;
use obj::{load_obj, FromRawVertex, Obj};
use pipeline::Pipeline;
use renderer::{Renderer, MAX_FRAMES_IN_FLIGHT};

use geometric_algebra::{rotor::Rotor, vector::Vector};

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: Vector<f32>,
    pub normal: Vector<f32>,
}

impl<I: Copy + num_traits::cast::FromPrimitive> FromRawVertex<I> for Vertex {
    fn process(
        vertices: Vec<(f32, f32, f32, f32)>,
        normals: Vec<(f32, f32, f32)>,
        tex_coords: Vec<(f32, f32, f32)>,
        polygons: Vec<obj::raw::object::Polygon>,
    ) -> obj::ObjResult<(Vec<Self>, Vec<I>)> {
        let (v, i) = obj::Vertex::process(vertices, normals, tex_coords, polygons)?;

        Ok((
            v.iter()
                .map(|v| Vertex {
                    pos: Vector::from_slice(v.position),
                    normal: Vector::from_slice(v.normal),
                })
                .collect::<Vec<_>>(),
            i,
        ))
    }
}

impl Vertex {
    pub const fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }

    pub const fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Self, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Self, normal) as u32,
            },
        ]
    }
}

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

#[repr(C)]
pub struct VertexPushConstants {
    camera_rotation: Rotor<f32>,
    camera_position: Vector<f32>,
}

#[repr(C)]
pub struct FragmentPushConstants {
    camera_vector: Vector<f32>,
}

#[repr(C)]
pub struct PushConstants {
    vertex: VertexPushConstants,
    _align: f32,
    fragment: FragmentPushConstants,
}

fn main() {
    env_logger::init();
    let mut gfx = Renderer::new(WIDTH, HEIGHT);

    let teapot: Obj<Vertex, u32> = load_obj(BufReader::new(
        File::open("newell_teaset/teapot-triangulated.obj").unwrap(),
    ))
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

    event_loop.run(|ctx: &mut EventLoop| {
        let camera_rotation = Vector::<f32>::E2
            .wedge(Vector::<f32>::E3)
            .rotor(-std::f32::consts::PI / 8.0)
            * Vector::<f32>::E3
                .wedge(Vector::<f32>::E1)
                .rotor(ctx.time + std::f32::consts::PI / 2.0);

        let push_constants = PushConstants {
            vertex: VertexPushConstants {
                camera_position: Vector::<f32>::new(
                    10.0 * (ctx.time.cos()),
                    6.0,
                    10.0 * (-ctx.time.sin()),
                ),
                camera_rotation,
            },
            _align: 0.0,
            fragment: FragmentPushConstants {
                camera_vector: Vector::<f32>::new(1.0, 1.0, 1.0)
                    .norm()
                    .rotate(camera_rotation.conjugate()),
            },
        };

        let current_frame = gfx.current_frame;
        unsafe {
            let fence = &[*gfx.in_flight_fences[current_frame]];
            gfx.device.wait_for_fences(fence, true, u64::MAX).unwrap();

            let (image_index, recreate_swapchain) = match (
                gfx.swapchain.loader.acquire_next_image(
                    *gfx.swapchain,
                    u64::MAX,
                    *gfx.image_avaliable_semaphores[current_frame],
                    vk::Fence::null(),
                ),
                ctx.framebuffer_resized,
            ) {
                (Ok((image_index, true)), _) | (Ok((image_index, false)), true) => {
                    (image_index, true)
                }
                (Ok((image_index, false)), false) => (image_index, false),
                (Err(vk::Result::ERROR_OUT_OF_DATE_KHR), _) => {
                    ctx.framebuffer_resized = false;
                    gfx.recreate_swapchain();
                    return;
                }
                (Err(_), _) => {
                    panic!("failed to acquire swapchain image")
                }
            };

            gfx.device.reset_fences(fence).unwrap();

            take_mut::take(
                gfx.command_buffers.get_mut(current_frame).unwrap(),
                |command_buffer| {
                    command_buffer
                        .begin()
                        .record(|command_buffer| {
                            record_command_buffer(
                                command_buffer,
                                &gfx.device,
                                &gfx.swapchain.pipeline,
                                &gfx.swapchain.images[image_index as usize],
                                &vertex_buffer,
                                &index_buffer,
                                teapot.indices.len().try_into().unwrap(),
                                push_constants,
                            )
                        })
                        .end()
                        .submit(
                            gfx.device.graphics_queue,
                            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                            *gfx.image_avaliable_semaphores[current_frame],
                            *gfx.render_finished_semaphores[current_frame],
                            *gfx.in_flight_fences[current_frame],
                        )
                },
            );

            let swapchains = [*gfx.swapchain];
            let indices: [u32; 1] = [image_index];

            let wait_semaphore = [*gfx.render_finished_semaphores[current_frame]];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait_semaphore)
                .swapchains(&swapchains)
                .image_indices(&indices);

            match gfx
                .swapchain
                .loader
                .queue_present(gfx.device.present_queue, &present_info)
            {
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => gfx.recreate_swapchain(),
                Err(e) => panic!("{}", e),
                _ => (),
            };

            if recreate_swapchain {
                ctx.framebuffer_resized = false;
                gfx.recreate_swapchain();
            }
        }

        gfx.current_frame = (gfx.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    });

    gfx.wait_idle();
}

#[allow(clippy::too_many_arguments)]
pub fn record_command_buffer(
    command_buffer: ActiveMultipleSubmitCommandBuffer,
    device: &Device,
    pipeline: &Pipeline,
    image: &SwapchainImage,
    vertex_buffer: &Buffer<Vertex>,
    index_buffer: &Buffer<u32>,
    index_count: u32,
    push_constants: PushConstants,
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
        device.cmd_draw_indexed(cmd_buf, index_count, 1, 0, 0, 0);

        device.cmd_end_render_pass(cmd_buf);

        command_buffer
    }
}
