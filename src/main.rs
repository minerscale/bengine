pub mod buffer;
pub mod command_buffer;
pub mod debug_messenger;
pub mod device;
pub mod instance;
pub mod pipeline;
pub mod render_pass;
pub mod shader_module;
pub mod surface;
pub mod swapchain;
pub mod synchronization;
pub mod vertex;

use std::mem::offset_of;

use ash::vk;
use buffer::Buffer;
use command_buffer::CommandBuffer;
use command_buffer::CommandPool;
use debug_messenger::DebugMessenger;
use device::Device;
use instance::Instance;
use pipeline::Pipeline;
use sdl2::{keyboard::Keycode, sys::SDL_Vulkan_GetDrawableSize};
use surface::Surface;
use swapchain::Swapchain;
use swapchain::SwapchainImage;
use synchronization::{Fence, Semaphore};

const MAX_FRAMES_IN_FLIGHT: usize = 2;
const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);

use glam::Vec2;
use glam::Vec3;
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Vertex {
    pub pos: Vec2,
    pub color: Vec3,
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
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Self, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Self, color) as u32,
            },
        ]
    }
}

fn main() {
    env_logger::init();
    let mut gfx = Graphics::new(800, 600);

    let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];

    let vertices = [
        Vertex {
            pos: Vec2::new(-0.5, -0.5),
            color: Vec3::new(1.0, 0.0, 0.0),
        },
        Vertex {
            pos: Vec2::new(0.5, -0.5),
            color: Vec3::new(0.0, 1.0, 0.0),
        },
        Vertex {
            pos: Vec2::new(0.5, 0.5),
            color: Vec3::new(0.0, 0.0, 1.0),
        },
        Vertex {
            pos: Vec2::new(-0.5, 0.5),
            color: Vec3::new(1.0, 1.0, 1.0),
        },
    ];

    let vertex_buffer = {
        let staging_buffer = Buffer::new(
            gfx.device.device.clone(),
            &gfx.instance,
            gfx.device.physical_device,
            &vertices,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        Buffer::copy_buffer(
            &gfx.instance,
            gfx.device.physical_device,
            gfx.device.graphics_queue,
            &gfx.command_pool,
            &staging_buffer,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
    };

    let index_buffer = {
        let staging_buffer = Buffer::new(
            gfx.device.device.clone(),
            &gfx.instance,
            gfx.device.physical_device,
            &indices,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        Buffer::copy_buffer(
            &gfx.instance,
            gfx.device.physical_device,
            gfx.device.graphics_queue,
            &gfx.command_pool,
            &staging_buffer,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
    };

    let f = |gfx: &mut Graphics| {
        let current_frame = gfx.current_frame;
        unsafe {
            let fence = &[*gfx.in_flight_fences[current_frame]];
            gfx.device.wait_for_fences(fence, true, u64::MAX).unwrap();

            let (image_index, recreate_swapchain) = match (
                gfx.swapchain.device.acquire_next_image(
                    gfx.swapchain.clone(),
                    u64::max_value(),
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

            let command_buffer = &gfx.command_buffers[current_frame];

            record_command_buffer(
                &gfx.device,
                &gfx.pipeline,
                command_buffer,
                &gfx.swapchain.images[image_index as usize],
                &vertex_buffer,
                &index_buffer,
            );

            let image_avaliable_semaphore = [*gfx.image_avaliable_semaphores[current_frame]];
            let render_finished_semaphore = [*gfx.render_finished_semaphores[current_frame]];
            let command_buffer_list = [**command_buffer];

            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&image_avaliable_semaphore)
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .command_buffers(&command_buffer_list)
                .signal_semaphores(&render_finished_semaphore);

            gfx.device
                .queue_submit(
                    gfx.device.graphics_queue,
                    &[submit_info],
                    *gfx.in_flight_fences[current_frame],
                )
                .unwrap();

            let swapchains = [*gfx.swapchain];
            let indices: [u32; 1] = [image_index];

            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&render_finished_semaphore)
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

pub fn record_command_buffer(
    device: &Device,
    pipeline: &Pipeline,
    command_buffer: &CommandBuffer,
    image: &SwapchainImage,
    vertex_buffer: &Buffer<Vertex>,
    index_buffer: &Buffer<u16>,
) {
    unsafe {
        command_buffer.begin(vk::CommandBufferUsageFlags::empty());

        let clear_color = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        }];

        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(*pipeline.render_pass)
            .framebuffer(image.framebuffer.unwrap())
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: image.extent,
            })
            .clear_values(&clear_color);

        let cmd_buf = **command_buffer;

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
        device.cmd_bind_index_buffer(cmd_buf, index_buffer.buffer, 0, vk::IndexType::UINT16);

        device.cmd_draw_indexed(cmd_buf, 6, 1, 0, 0, 0);

        device.cmd_end_render_pass(cmd_buf);

        command_buffer.end();
    }
}

pub struct Graphics {
    // WARNING: Cleanup order matters here
    pub image_avaliable_semaphores: Vec<Semaphore>,
    pub render_finished_semaphores: Vec<Semaphore>,
    pub in_flight_fences: Vec<Fence>,

    pub command_buffers: Vec<CommandBuffer>,
    pub command_pool: CommandPool,

    pub pipeline: Pipeline,
    pub swapchain: Swapchain,
    pub device: Device,

    pub surface: Surface,

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

        let mut swapchain = Swapchain::new(
            &self.instance,
            &self.device,
            &self.surface.loader,
            *self.surface,
            extent,
            Some(*self.swapchain),
        );

        swapchain.attach_framebuffers(&self.pipeline);

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

        let mut swapchain = Swapchain::new(
            &instance,
            &device,
            &surface.loader,
            *surface,
            vk::Extent2D { width, height },
            None,
        );

        let pipeline = Pipeline::new(device.device.clone(), &swapchain);

        swapchain.attach_framebuffers(&pipeline);

        let command_pool = CommandPool::new(&device);

        let mut image_avaliable_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut render_finished_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut in_flight_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);

        let mut command_buffers = Vec::new();

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            command_buffers.push(command_pool.create_command_buffer());

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
            pipeline,
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

    pub fn render_loop<F: Fn(&mut Graphics)>(&mut self, f: F) {
        let mut event_pump = self.sdl_context.event_pump().unwrap();

        use sdl2::event::Event;

        //let mut frame_count = 0;
        //let begin_time = std::time::Instant::now();
        'quit: loop {
            f(self);

            while let Some(event) = event_pump.poll_event() {
                match event {
                    Event::Quit { timestamp: _ } => break 'quit,
                    Event::KeyDown {
                        timestamp: _,
                        window_id: _,
                        keycode,
                        scancode: _,
                        keymod: _,
                        repeat: _,
                    } => {
                        if let Some(key) = keycode {
                            match key {
                                Keycode::Escape => break 'quit,
                                _ => (),
                            }
                        }
                    }
                    Event::Window {
                        timestamp: _,
                        window_id: _,
                        win_event,
                    } => match win_event {
                        sdl2::event::WindowEvent::SizeChanged(_, _) => {
                            self.framebuffer_resized = true
                        }
                        _ => (),
                    },

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
