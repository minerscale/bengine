pub mod buffer;
pub mod command_buffer;
pub mod debug_messenger;
pub mod device;
pub mod image;
pub mod instance;
pub mod pipeline;
pub mod render_pass;
pub mod shader_module;
pub mod surface;
pub mod swapchain;
pub mod synchronization;

use std::{fs::File, io::BufReader, mem::offset_of, ptr::addr_of};

use ash::vk;
use buffer::{Buffer, StagedBuffer};
use command_buffer::{ActiveMultipleSubmitCommandBuffer, CommandPool, MultipleSubmitCommandBuffer};

use debug_messenger::DebugMessenger;
use device::Device;
use image::SwapchainImage;
use instance::Instance;
use obj::{load_obj, FromRawVertex, Obj};
use pipeline::Pipeline;
use sdl2::{keyboard::Keycode, sys::SDL_Vulkan_GetDrawableSize};
use surface::Surface;
use swapchain::Swapchain;
use synchronization::{Fence, Semaphore};

const MAX_FRAMES_IN_FLIGHT: usize = 2;
const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);

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

pub struct Context {
    time: f32,
}

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

impl Context {
    fn new() -> Self {
        Self { time: 0.0 }
    }
}

fn main() {
    env_logger::init();
    let mut gfx = Graphics::new(WIDTH, HEIGHT);

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

    let f = |gfx: &mut Graphics, ctx: &mut Context| {
        /*let transformed = {
            ctx.time += 1.0 / 60.0;
            ctx.camera_rotation = Vector::<f32>::E2.wedge(Vector::<f32>::E3).rotor(ctx.time);
            ctx.camera_position.e1 = ctx.time.sin();

            teapot
                .vertices
                .iter()
                .map(|vtx| Vertex {
                    pos: {
                        let d = (vtx.pos - ctx.camera_position).rotate(ctx.camera_rotation);
                        Vector::<f32>::new(
                            ctx.camera_depth * (d.e1 / d.e3),
                            ctx.camera_depth * (d.e2 / d.e3) * (gfx.swapchain.extent.width as f32)
                                / (gfx.swapchain.extent.height as f32),
                            d.e3,
                        )
                    },
                    normal: vtx.normal,
                })
                .collect::<Vec<_>>()
        };*/

        let camera_rotation = Vector::<f32>::E2
            .wedge(Vector::<f32>::E3)
            .rotor(-std::f32::consts::PI / 8.0)
            * Vector::<f32>::E3
                .wedge(Vector::<f32>::E1)
                .rotor(ctx.time + std::f32::consts::PI / 2.0);

        ctx.time += 1.0 / 60.0;
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
                gfx.swapchain.device.acquire_next_image(
                    *gfx.swapchain,
                    u64::MAX,
                    *gfx.image_avaliable_semaphores[current_frame],
                    vk::Fence::null(),
                ),
                gfx.framebuffer_resized,
            ) {
                (Ok((image_index, true)), _) | (Ok((image_index, false)), true) => {
                    (image_index, true)
                }
                (Ok((image_index, false)), false) => (image_index, false),
                (Err(vk::Result::ERROR_OUT_OF_DATE_KHR), _) => {
                    gfx.framebuffer_resized = false;
                    gfx.recreate_swapchain();
                    return;
                }
                (Err(_), _) => {
                    panic!("failed to acquire swapchain image")
                }
            };

            gfx.device.reset_fences(fence).unwrap();

            let command_buffer = match gfx.command_buffers[current_frame].take() {
                Some(buf) => buf.begin(),
                None => panic!("Attempt to begin an active command buffer!"),
            };

            //vertex_buffer.upload_new(&transformed, 0, &command_buffer);

            gfx.command_buffers[current_frame] = Some(
                record_command_buffer(
                    &gfx.device,
                    &gfx.swapchain.pipeline,
                    command_buffer,
                    &gfx.swapchain.images[image_index as usize],
                    &vertex_buffer,
                    &index_buffer,
                    teapot.indices.len().try_into().unwrap(),
                    push_constants,
                )
                .end()
                .submit(
                    gfx.device.graphics_queue,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    *gfx.image_avaliable_semaphores[current_frame],
                    *gfx.render_finished_semaphores[current_frame],
                    *gfx.in_flight_fences[current_frame],
                ),
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
                .device
                .queue_present(gfx.device.present_queue, &present_info)
            {
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => gfx.recreate_swapchain(),
                Err(e) => panic!("{}", e),
                _ => (),
            };

            if recreate_swapchain {
                gfx.framebuffer_resized = false;
                gfx.recreate_swapchain();
            }
        }

        gfx.current_frame = (gfx.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    };

    gfx.render_loop(f);

    gfx.wait_idle();
}

#[allow(clippy::too_many_arguments)]
pub fn record_command_buffer(
    device: &Device,
    pipeline: &Pipeline,
    command_buffer: ActiveMultipleSubmitCommandBuffer,
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
            .framebuffer(image.framebuffer.unwrap())
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

pub struct Graphics {
    // WARNING: Cleanup order matters here
    pub image_avaliable_semaphores: Vec<Semaphore>,
    pub render_finished_semaphores: Vec<Semaphore>,
    pub in_flight_fences: Vec<Fence>,

    pub command_buffers: Vec<Option<MultipleSubmitCommandBuffer>>,
    pub command_pool: CommandPool,

    pub swapchain: Swapchain,

    pub device: Device,

    pub surface: Surface,

    //pub depth_image: Image,
    pub debug_callback: Option<DebugMessenger>,
    pub instance: Instance,

    // the SDL context must outlive the swapchain
    pub sdl_context: sdl2::Sdl,
    pub window: sdl2::video::Window,

    pub entry: ash::Entry,

    pub framebuffer_resized: bool,
    pub current_frame: usize,
}

impl Graphics {
    pub fn wait_idle(&self) {
        unsafe { self.device.device_wait_idle().unwrap() };
    }

    pub fn recreate_swapchain(&mut self) {
        let mut width: std::ffi::c_int = 0;
        let mut height: std::ffi::c_int = 0;

        unsafe {
            SDL_Vulkan_GetDrawableSize(
                self.window.raw(),
                (&mut width) as *mut std::ffi::c_int,
                (&mut height) as *mut std::ffi::c_int,
            )
        };

        let extent = vk::Extent2D {
            width: width.try_into().unwrap(),
            height: height.try_into().unwrap(),
        };

        self.wait_idle();

        let swapchain = Swapchain::new(
            &self.instance,
            &self.device,
            &self.surface.loader,
            *self.surface,
            extent,
            Some(*self.swapchain),
        );

        self.swapchain = swapchain;
    }

    pub fn new(width: u32, height: u32) -> Self {
        let entry = ash::Entry::linked();

        let sdl_context = sdl2::init().unwrap();
        let window = {
            sdl_context
                .video()
                .unwrap()
                .window("Space Game", width, height)
                .allow_highdpi()
                .vulkan()
                .position_centered()
                .resizable()
                .build()
                .map_err(|e| e.to_string())
                .unwrap()
        };

        let instance = Instance::new(&entry, &window);

        let debug_callback = if ENABLE_VALIDATION_LAYERS {
            Some(DebugMessenger::new(&entry, &instance))
        } else {
            None
        };

        let surface = Surface::new(&entry, &window, &instance);

        let device = Device::new(&instance, &surface.loader, *surface);

        let swapchain = Swapchain::new(
            &instance,
            &device,
            &surface.loader,
            *surface,
            vk::Extent2D { width, height },
            None,
        );

        let command_pool = CommandPool::new(&device);

        let mut image_avaliable_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut render_finished_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut in_flight_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);

        let mut command_buffers = Vec::new();

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            command_buffers.push(Some(command_pool.create_command_buffer()));

            image_avaliable_semaphores.push(Semaphore::new(device.device.clone()));
            render_finished_semaphores.push(Semaphore::new(device.device.clone()));
            in_flight_fences.push(Fence::new(device.device.clone()));
        }

        Self {
            image_avaliable_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            command_buffers,
            command_pool,
            swapchain,
            device,
            surface,
            debug_callback,
            instance,
            sdl_context,
            window,
            entry,
            framebuffer_resized: false,
            current_frame: 0,
        }
    }

    pub fn render_loop<F: Fn(&mut Graphics, &mut Context)>(&mut self, f: F) {
        let mut event_pump = self.sdl_context.event_pump().unwrap();

        use sdl2::event::Event;

        //let mut frame_count = 0;
        //let begin_time = std::time::Instant::now();
        let mut context = Context::new();

        'quit: loop {
            f(self, &mut context);

            while let Some(event) = event_pump.poll_event() {
                match event {
                    Event::Quit { timestamp: _ } => break 'quit,
                    Event::KeyDown {
                        timestamp: _,
                        window_id: _,
                        keycode: Some(Keycode::Escape),
                        scancode: _,
                        keymod: _,
                        repeat: _,
                    } => break 'quit,
                    Event::Window {
                        timestamp: _,
                        window_id: _,
                        win_event: sdl2::event::WindowEvent::SizeChanged(_, _),
                    } => self.framebuffer_resized = true,
                    _ => (),
                }
            }

            //frame_count += 1;
            //let time_elapsed = std::time::Instant::now().duration_since(begin_time);

            //let framerate = time_elapsed / frame_count;
            //println!("{}", 1f64/framerate.as_secs_f64());
        }
    }
}
