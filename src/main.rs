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

use ash::vk;
use command_buffer::CommandPool;
use debug_messenger::DebugMessenger;
use device::Device;
use instance::Instance;
use pipeline::Pipeline;
use sdl2::{keyboard::Keycode, sys::SDL_Vulkan_GetDrawableSize};
use surface::Surface;
use swapchain::Swapchain;
use synchronization::{Fence, Semaphore};

const MAX_FRAMES_IN_FLIGHT: usize = 2;
const ENABLE_VALIDATION_LAYERS: bool = true;

fn main() {
    env_logger::init();
    let mut gfx = Graphics::new(800, 600);

    fn f(gfx: &mut Graphics) {
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

            let command_buffer = gfx.command_pool.command_buffers[current_frame].clone();

            let image_avaliable_semaphore = [*gfx.image_avaliable_semaphores[current_frame]];
            let render_finished_semaphore = [*gfx.render_finished_semaphores[current_frame]];

            command_buffer.record(image_index, &gfx.pipeline, &gfx.swapchain);
            command_buffer.submit(
                gfx.device.present_queue,
                &image_avaliable_semaphore,
                &render_finished_semaphore,
                *gfx.in_flight_fences[current_frame],
            );

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
    }

    gfx.render_loop(f);

    gfx.wait_idle();
}

pub struct Graphics {
    // WARNING: Cleanup order matters here
    pub image_avaliable_semaphores: Vec<Semaphore>,
    pub render_finished_semaphores: Vec<Semaphore>,
    pub in_flight_fences: Vec<Fence>,

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

        println!("{extent:?}");

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

        let debug_callback =
            ENABLE_VALIDATION_LAYERS.then_some(DebugMessenger::new(&entry, &instance));

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

        let pipeline = Pipeline::new(&device, &swapchain);

        swapchain.attach_framebuffers(&pipeline);

        let mut command_pool = CommandPool::new(&device);

        let mut image_avaliable_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut render_finished_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut in_flight_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            command_pool.push_command_buffer();

            image_avaliable_semaphores.push(Semaphore::new(device.clone()));
            render_finished_semaphores.push(Semaphore::new(device.clone()));
            in_flight_fences.push(Fence::new(device.clone()));
        }

        Self {
            image_avaliable_semaphores,
            render_finished_semaphores,
            in_flight_fences,
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

            //std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
