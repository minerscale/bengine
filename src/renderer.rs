use ash::vk;
use sdl2::sys::SDL_Vulkan_GetDrawableSize;

use crate::{
    command_buffer::{ActiveMultipleSubmitCommandBuffer, CommandPool, MultipleSubmitCommandBuffer},
    debug_messenger::{DebugMessenger, ENABLE_VALIDATION_LAYERS},
    device::Device,
    image::SwapchainImage,
    instance::Instance,
    pipeline::Pipeline,
    surface::Surface,
    swapchain::Swapchain,
    synchronization::{Fence, Semaphore},
};

pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct Renderer {
    // WARNING: Cleanup order matters here
    pub image_avaliable_semaphores: Vec<Semaphore>,
    pub render_finished_semaphores: Vec<Semaphore>,
    pub in_flight_fences: Vec<Fence>,

    pub command_buffers: Vec<MultipleSubmitCommandBuffer>,
    pub command_pool: CommandPool,

    pub swapchain: Swapchain,

    pub device: Device,

    pub surface: Surface,

    pub debug_callback: Option<DebugMessenger>,
    pub instance: Instance,

    pub entry: ash::Entry,

    pub window: sdl2::video::Window,
    pub sdl_context: sdl2::Sdl,

    pub current_frame: usize,
}

impl Renderer {
    pub fn wait_idle(&self) {
        unsafe { self.device.device_wait_idle().unwrap() };
    }

    pub fn draw<
        F: FnMut(
            &Device,
            &Pipeline,
            ActiveMultipleSubmitCommandBuffer,
            &SwapchainImage,
        ) -> ActiveMultipleSubmitCommandBuffer,
    >(
        &mut self,
        mut record_command_buffer: F,
        framebuffer_resized: &bool,
    ) -> bool {
        unsafe {
            let fence = &[*self.in_flight_fences[self.current_frame]];
            self.device.wait_for_fences(fence, true, u64::MAX).unwrap();

            let (image_index, mut recreate_swapchain) = match (
                self.swapchain.loader.acquire_next_image(
                    *self.swapchain,
                    u64::MAX,
                    *self.image_avaliable_semaphores[self.current_frame],
                    vk::Fence::null(),
                ),
                framebuffer_resized,
            ) {
                (Ok((image_index, true)), _) | (Ok((image_index, false)), true) => {
                    (image_index, true)
                }
                (Ok((image_index, false)), false) => (image_index, false),
                (Err(vk::Result::ERROR_OUT_OF_DATE_KHR), _) => {
                    self.recreate_swapchain();
                    return false;
                }
                (Err(_), _) => {
                    panic!("failed to acquire swapchain image")
                }
            };

            self.device.reset_fences(fence).unwrap();

            take_mut::take(
                self.command_buffers.get_mut(self.current_frame).unwrap(),
                |command_buffer| {
                    command_buffer
                        .begin()
                        .record(|command_buffer| {
                            record_command_buffer(
                                &self.device,
                                &self.swapchain.pipeline,
                                command_buffer,
                                &self.swapchain.images[image_index as usize],
                            )
                        })
                        .end()
                        .submit(
                            self.device.graphics_queue,
                            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                            *self.image_avaliable_semaphores[self.current_frame],
                            *self.render_finished_semaphores[self.current_frame],
                            *self.in_flight_fences[self.current_frame],
                        )
                },
            );

            let swapchains = [*self.swapchain];
            let indices: [u32; 1] = [image_index];

            let wait_semaphore = [*self.render_finished_semaphores[self.current_frame]];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait_semaphore)
                .swapchains(&swapchains)
                .image_indices(&indices);

            match self
                .swapchain
                .loader
                .queue_present(self.device.present_queue, &present_info)
            {
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => recreate_swapchain = true,
                Err(e) => panic!("{}", e),
                _ => (),
            };

            if recreate_swapchain {
                self.recreate_swapchain();
            }
        }

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;

        false
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
            Some(&self.swapchain),
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
                .window("bengine", width, height)
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

        let device = Device::new(&instance, &surface);

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
            swapchain,
            device,
            surface,
            debug_callback,
            instance,
            sdl_context,
            window,
            entry,
            current_frame: 0,
        }
    }
}
