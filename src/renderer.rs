use ash::vk;
use pipeline::Pipeline;
use render_pass::RenderPass;
use ultraviolet::Isometry3;

pub mod buffer;
pub mod command_buffer;
pub mod device;
pub mod image;
pub mod pipeline;
pub mod render_pass;
pub mod sampler;
pub mod texture;
pub mod shader_module;

mod debug_messenger;
mod descriptors;
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
pub const FOV: f32 = 90.0;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default)]
pub struct UniformBufferObject {
    pub view_transform: Isometry3,
    pub time: f32
}

type PipelineFunction = fn(
                &Device,
                vk::Extent2D,
                vk::RenderPass,
                &[vk::DescriptorSetLayout],
            ) -> Pipeline;

pub struct Renderer {
    // WARNING: Cleanup order matters here
    image_avaliable_semaphores: Vec<Semaphore>,
    render_finished_semaphores: Vec<Semaphore>,
    in_flight_fences: Vec<Fence>,
    semaphore_ready_fences: Vec<Fence>,

    uniform_buffer_layout: DescriptorSetLayout,
    pub texture_layout: DescriptorSetLayout,
    pub descriptor_pool: DescriptorPool,
    uniform_buffers: Vec<MappedBuffer<UniformBufferObject>>,

    command_buffers: Vec<MultipleSubmitCommandBuffer>,
    pub command_pool: CommandPool,

    pipelines: &'static [PipelineFunction],

    swapchain: Swapchain,

    pub device: Device,

    surface: Surface,

    #[allow(dead_code)]
    debug_callback: Option<DebugMessenger>,

    pub instance: Instance,

    #[allow(dead_code)]
    entry: ash::Entry,

    pub window: sdl3::video::Window,
    pub sdl_context: sdl3::Sdl,

    current_frame: usize,
}

impl Renderer {
    pub fn wait_idle(&self) {
        unsafe { self.device.device_wait_idle().unwrap() };
    }

    pub fn draw<
        F: FnMut(
            &Device,
            &RenderPass,
            ActiveMultipleSubmitCommandBuffer,
            &mut MappedBuffer<UniformBufferObject>,
            &SwapchainImage,
        ) -> ActiveMultipleSubmitCommandBuffer,
    >(
        &mut self,
        mut record_command_buffer: F,
        framebuffer_resized: bool,
    ) -> bool {
        unsafe {
            let fences = &[
                *self.in_flight_fences[self.current_frame],
                *self.semaphore_ready_fences[self.current_frame],
            ];
            self.device.wait_for_fences(fences, true, u64::MAX).unwrap();

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

            self.device.reset_fences(fences).unwrap();

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
                            *self.render_finished_semaphores[self.current_frame],
                            *self.in_flight_fences[self.current_frame],
                        )
                },
            );

            let swapchains = [*self.swapchain];
            let indices: [u32; 1] = [image_index];

            let semaphore_ready_fence = [*self.semaphore_ready_fences[self.current_frame]];
            let mut fence_info =
                vk::SwapchainPresentFenceInfoEXT::default().fences(&semaphore_ready_fence);

            let wait_semaphore = [*self.render_finished_semaphores[self.current_frame]];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait_semaphore)
                .swapchains(&swapchains)
                .image_indices(&indices)
                .push_next(&mut fence_info);

            match self
                .swapchain
                .loader
                .queue_present(self.device.present_queue, &present_info)
            {
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    recreate_swapchain = true;
                }
                Err(e) => panic!("{}", e),
                _ => (),
            }

            if recreate_swapchain {
                self.recreate_swapchain();
            }
        }

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;

        false
    }

    pub fn recreate_swapchain(&mut self) {
        let extent = if cfg!(target_os = "macos") {
            (WIDTH, HEIGHT)
        } else {
            self.window.size_in_pixels()
        };

        let extent = vk::Extent2D {
            width: extent.0,
            height: extent.1,
        };

        self.wait_idle();

        let descriptor_set_layouts = [
            self.uniform_buffer_layout.layout,
            self.texture_layout.layout,
        ];

        let swapchain = Swapchain::new(
            &self.instance,
            &self.device,
            &self.surface.loader,
            *self.surface,
            extent,
            &descriptor_set_layouts,
            self.pipelines.iter(),
            Some(&self.swapchain),
        );

        self.swapchain = swapchain;
    }

    pub fn new(width: u32, height: u32, pipelines: &'static [PipelineFunction]) -> Self {
        let entry = ash::Entry::linked();

        let sdl_context = sdl3::init().unwrap();
        let window = {
            sdl_context
                .video()
                .unwrap()
                .window("bengine", width, height)
                .vulkan()
                .position_centered()
                .resizable()
                .build()
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

        let uniform_buffer_layout = {
            let uniform_buffer_bindings = [vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)];

            DescriptorSetLayout::new(device.device.clone(), &uniform_buffer_bindings)
        };

        let texture_layout = {
            let texture_bindings = [vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)];

            DescriptorSetLayout::new(device.device.clone(), &texture_bindings)
        };

        let descriptor_set_layouts = [uniform_buffer_layout.layout, texture_layout.layout];

        let swapchain = Swapchain::new(
            &instance,
            &device,
            &surface.loader,
            *surface,
            vk::Extent2D { width, height },
            &descriptor_set_layouts,
            pipelines.iter(),
            None,
        );

        let command_pool = CommandPool::new(&device);

        let mut image_avaliable_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut render_finished_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut in_flight_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
        let mut semaphore_ready_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);

        let mut command_buffers = Vec::new();
        let mut uniform_buffers = Vec::new();

        let descriptor_pool = DescriptorPool::new(device.device.clone());

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            command_buffers.push(command_pool.create_command_buffer());

            image_avaliable_semaphores.push(Semaphore::new(device.device.clone()));
            render_finished_semaphores.push(Semaphore::new(device.device.clone()));
            in_flight_fences.push(Fence::new(device.device.clone()));
            semaphore_ready_fences.push(Fence::new(device.device.clone()));

            uniform_buffers.push(MappedBuffer::new(
                &device.device,
                &instance,
                device.physical_device,
                &[UniformBufferObject::default()],
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                &descriptor_pool,
                &uniform_buffer_layout,
            ));
        }

        Self {
            image_avaliable_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            semaphore_ready_fences,
            descriptor_pool,
            uniform_buffer_layout,
            texture_layout,
            uniform_buffers,
            command_buffers,
            command_pool,
            pipelines,
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
