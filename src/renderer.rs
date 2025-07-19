use std::{iter::repeat_with, sync::Arc};

use ash::vk;
use pipeline::Pipeline;
use render_pass::RenderPass;
use ultraviolet::Isometry3;

pub mod buffer;
pub mod command_buffer;
pub mod descriptors;
pub mod device;
pub mod image;
pub mod material;
pub mod pipeline;
pub mod render_pass;
pub mod sampler;
pub mod shader_module;

mod debug_messenger;
mod instance;
mod surface;
mod swapchain;
mod synchronization;

use crate::renderer::{
    buffer::MappedBuffer,
    command_buffer::{ActiveMultipleSubmitCommandBuffer, CommandPool, MultipleSubmitCommandBuffer},
    debug_messenger::{DebugMessenger, ENABLE_VALIDATION_LAYERS},
    descriptors::{DescriptorPool, DescriptorSetLayout},
    device::Device,
    image::SwapchainImage,
    instance::Instance,
    surface::Surface,
    swapchain::Swapchain,
    synchronization::{Fence, Semaphore},
};

pub const MAX_FRAMES_IN_FLIGHT: usize = 2;
pub const WIDTH: u32 = 800;
pub const HEIGHT: u32 = 600;

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct UniformBufferObject {
    pub view_transform: Isometry3,
    pub time: f32,
    pub fov: f32,
    pub scale_y: f32,
}

pub type PipelineFunction = for<'a, 'b> fn(
    &'a device::Device,
    vk::Extent2D,
    ash::vk::RenderPass,
    &'b [ash::vk::DescriptorSetLayout],
) -> Pipeline;

pub type DescriptorSetLayoutFunction = fn(Arc<ash::Device>) -> DescriptorSetLayout;

enum ImageIndex {
    Acquiring,
    Recording(u32),
    Presenting(u32),
}

pub struct Renderer {
    // WARNING: Cleanup order matters here
    image_avaliable_semaphores: Box<[Semaphore]>,
    render_finished_semaphores: Box<[Semaphore]>,
    in_flight_fences: Box<[Fence]>,

    image_index: ImageIndex,
    recreate_swapchain: bool,
    window_size: (u32, u32),

    pub descriptor_set_layouts: Box<[DescriptorSetLayout]>,

    pub descriptor_pool: DescriptorPool,
    uniform_buffers: Box<[Box<[MappedBuffer<UniformBufferObject>]>]>,

    command_buffers: Box<[MultipleSubmitCommandBuffer]>,
    pub command_pool: CommandPool,

    pipelines: &'static [PipelineFunction],

    pub swapchain: Swapchain,

    pub device: Device,

    surface: Surface,

    #[allow(dead_code)]
    debug_callback: Option<DebugMessenger>,

    pub instance: Instance,

    #[allow(dead_code)]
    entry: ash::Entry,

    current_frame: usize,
}

fn get_descriptor_set_layouts(layouts: &[DescriptorSetLayout]) -> Box<[vk::DescriptorSetLayout]> {
    layouts.iter().map(|layout| layout.layout).collect()
}

impl Renderer {
    pub fn wait_idle(&self) {
        unsafe { self.device.device_wait_idle().unwrap() };
    }

    pub fn update_window_size(&mut self, window: &sdl3::video::Window) {
        match window.size_in_pixels() {
            (x, y) if x > 1 && y > 1 => self.window_size = window.size_in_pixels(),
            _ => (),
        }
    }

    pub fn acquire_next_image(&mut self, mut framebuffer_resized: bool) {
        assert!(matches!(self.image_index, ImageIndex::Acquiring));

        let fences = &[*self.in_flight_fences[self.current_frame]];

        (self.image_index, self.recreate_swapchain) = loop {
            unsafe {
                self.device.wait_for_fences(fences, true, u64::MAX).unwrap();
            }
            match (
                unsafe {
                    self.swapchain.loader.acquire_next_image(
                        *self.swapchain,
                        u64::MAX,
                        *self.image_avaliable_semaphores[self.current_frame],
                        vk::Fence::null(),
                    )
                },
                framebuffer_resized,
            ) {
                (Ok((image_index, true)), _) | (Ok((image_index, false)), true) => {
                    break (ImageIndex::Recording(image_index), true);
                }
                (Ok((image_index, false)), false) => {
                    break (ImageIndex::Recording(image_index), false);
                }
                (Err(vk::Result::ERROR_OUT_OF_DATE_KHR), _) => {
                    self.recreate_swapchain();
                    framebuffer_resized = false;
                }
                (Err(_), _) => {
                    panic!("failed to acquire swapchain image")
                }
            };
        };

        unsafe {
            self.device.reset_fences(fences).unwrap();
        }
    }

    pub fn present(&mut self) {
        let image_index;
        (image_index, self.image_index) = match self.image_index {
            ImageIndex::Presenting(idx) => (idx, ImageIndex::Acquiring),
            _ => panic!("must draw image before presentation"),
        };

        let swapchains = [*self.swapchain];
        let indices: [u32; 1] = [image_index];

        let wait_semaphore = [*self.render_finished_semaphores[image_index as usize]];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphore)
            .swapchains(&swapchains)
            .image_indices(&indices);

        match unsafe {
            self.swapchain
                .loader
                .queue_present(self.device.present_queue, &present_info)
        } {
            Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain = true;
            }
            Err(e) => panic!("{}", e),
            _ => (),
        }

        if self.recreate_swapchain {
            self.recreate_swapchain();
        }
    }

    pub fn draw<
        F: FnMut(
            &Device,
            &RenderPass,
            ActiveMultipleSubmitCommandBuffer,
            &mut [MappedBuffer<UniformBufferObject>],
            &SwapchainImage,
        ) -> ActiveMultipleSubmitCommandBuffer,
    >(
        &mut self,
        mut record_command_buffer: F,
    ) {
        let image_index;
        (image_index, self.image_index) = match self.image_index {
            ImageIndex::Recording(idx) => (idx, ImageIndex::Presenting(idx)),
            _ => panic!("must acquire image before draw"),
        };

        replace_with::replace_with_or_abort(
            self.command_buffers.get_mut(self.current_frame).unwrap(),
            |command_buffer| {
                command_buffer
                    .begin()
                    .record(|command_buffer| {
                        record_command_buffer(
                            &self.device,
                            &self.swapchain.render_pass,
                            command_buffer,
                            &mut self.uniform_buffers[self.current_frame],
                            &self.swapchain.images[image_index as usize],
                        )
                    })
                    .end()
                    .submit(
                        self.device.graphics_queue,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        *self.image_avaliable_semaphores[self.current_frame],
                        *self.render_finished_semaphores[image_index as usize],
                        *self.in_flight_fences[self.current_frame],
                    )
            },
        );

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    }

    pub fn recreate_swapchain(&mut self) {
        let extent = self.window_size;

        let extent = vk::Extent2D {
            width: extent.0,
            height: extent.1,
        };

        self.wait_idle();

        let swapchain = Swapchain::new(
            &self.instance,
            &self.device,
            &self.surface.loader,
            *self.surface,
            extent,
            &get_descriptor_set_layouts(&self.descriptor_set_layouts),
            self.pipelines.iter(),
            Some(&self.swapchain),
        );

        self.wait_idle();

        self.swapchain = swapchain;
    }

    pub fn new(
        width: u32,
        height: u32,
        window: &sdl3::video::Window,
        descriptor_set_layouts: &[DescriptorSetLayoutFunction],
        pipelines: &'static [PipelineFunction],
    ) -> Self {
        let entry = ash::Entry::linked();

        let instance = Instance::new(&entry, &window);

        let debug_callback = if ENABLE_VALIDATION_LAYERS {
            Some(DebugMessenger::new(&entry, &instance))
        } else {
            None
        };

        let surface = Surface::new(&entry, &window, &instance);

        let device = Device::new(&instance, &surface);

        let descriptor_set_layouts = descriptor_set_layouts
            .iter()
            .map(|f| f(device.device.clone()))
            .collect::<Box<[_]>>();

        let descriptor_pool = DescriptorPool::new(device.device.clone());

        let uniform_buffers = with_n(
            || {
                descriptor_set_layouts
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, layout)| {
                        matches!(layout.descriptor_type, vk::DescriptorType::UNIFORM_BUFFER)
                            .then_some((idx, layout.binding))
                    })
                    .map(|(idx, binding)| {
                        MappedBuffer::new(
                            &device.device,
                            &instance,
                            device.physical_device,
                            &[UniformBufferObject::default()],
                            vk::BufferUsageFlags::UNIFORM_BUFFER,
                            vk::MemoryPropertyFlags::HOST_VISIBLE
                                | vk::MemoryPropertyFlags::HOST_COHERENT,
                            &descriptor_pool,
                            &descriptor_set_layouts[idx],
                            binding,
                        )
                    })
                    .collect::<Box<[_]>>()
            },
            MAX_FRAMES_IN_FLIGHT,
        );

        let swapchain = Swapchain::new(
            &instance,
            &device,
            &surface.loader,
            *surface,
            vk::Extent2D { width, height },
            &get_descriptor_set_layouts(&descriptor_set_layouts),
            pipelines.iter(),
            None,
        );

        let command_pool = CommandPool::new(&device);

        fn with_n<T, F: Fn() -> T>(f: F, n: usize) -> Box<[T]> {
            repeat_with(f).take(n).collect()
        }

        let image_avaliable_semaphores = with_n(
            || Semaphore::new(device.device.clone()),
            MAX_FRAMES_IN_FLIGHT,
        );
        let in_flight_fences = with_n(|| Fence::new(device.device.clone()), MAX_FRAMES_IN_FLIGHT);
        let command_buffers = with_n(
            || command_pool.create_command_buffer(),
            MAX_FRAMES_IN_FLIGHT,
        );
        let render_finished_semaphores = with_n(
            || Semaphore::new(device.device.clone()),
            swapchain.images.len(),
        );

        let image_index = ImageIndex::Acquiring;
        let recreate_swapchain = false;

        let window_size = (width, height);

        Self {
            image_avaliable_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            image_index,
            recreate_swapchain,
            window_size,
            descriptor_pool,
            descriptor_set_layouts,
            uniform_buffers,
            command_buffers,
            command_pool,
            pipelines,
            swapchain,
            device,
            surface,
            debug_callback,
            instance,
            entry,
            current_frame: 0,
        }
    }
}
