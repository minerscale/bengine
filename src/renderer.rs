use std::mem::offset_of;

use ash::vk;
use pipeline::{Pipeline, PipelineBuilder, VertexPushConstants};
use render_pass::RenderPass;
use shader_module::{SpecializationInfo, spv};
use ultraviolet::{Isometry3, Vec2, Vec4};
use vertex::Vertex;

pub mod buffer;
pub mod command_buffer;
pub mod device;
pub mod image;
pub mod mesh;
pub mod pipeline;
pub mod render_pass;
pub mod sampler;
pub mod texture;

mod debug_messenger;
mod descriptors;
mod instance;
mod shader_module;
mod surface;
mod swapchain;
mod synchronization;
mod vertex;

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

fn make_main_pipeline(
    device: &Device,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    descriptor_set_layouts: &[vk::DescriptorSetLayout],
) -> Pipeline {
    let fov = FOV.to_radians();
    let ez = f32::tan(fov / 2.0).recip();
    let camera_parameters = Vec4::new(
        ez,
        (extent.width as f32) / (extent.height as f32),
        0.01,
        1000.0,
    );

    let vertex_specialization = SpecializationInfo::new(
        &[
            vk::SpecializationMapEntry {
                constant_id: 0,
                offset: offset_of!(Vec4, x) as u32,
                size: std::mem::size_of::<f32>(),
            },
            vk::SpecializationMapEntry {
                constant_id: 1,
                offset: offset_of!(Vec4, y) as u32,
                size: std::mem::size_of::<f32>(),
            },
            vk::SpecializationMapEntry {
                constant_id: 2,
                offset: offset_of!(Vec4, z) as u32,
                size: std::mem::size_of::<f32>(),
            },
            vk::SpecializationMapEntry {
                constant_id: 3,
                offset: offset_of!(Vec4, w) as u32,
                size: std::mem::size_of::<f32>(),
            },
        ],
        unsafe {
            std::slice::from_raw_parts(
                (&raw const camera_parameters).cast::<u8>(),
                std::mem::size_of::<Vec4>(),
            )
        },
    );

    let shader_stages = [
        spv!(
            device.device.clone(),
            "shader.vert",
            vk::ShaderStageFlags::VERTEX,
            Some(vertex_specialization)
        ),
        spv!(
            device.device.clone(),
            "shader.frag",
            vk::ShaderStageFlags::FRAGMENT,
            None
        ),
    ];

    let push_constant_ranges = [vk::PushConstantRange::default()
        .offset(0)
        .size(
            std::mem::size_of::<VertexPushConstants>()
                .try_into()
                .unwrap(),
        )
        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)];

    let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
        .sample_shading_enable(false)
        .rasterization_samples(device.msaa_samples)
        .min_sample_shading(1.0);

    let viewport = [vk::Viewport::default()
        .x(0.0)
        .y(0.0)
        .width(extent.width as f32)
        .height(extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0)];

    let scissor = [vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent,
    }];

    let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false);

    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(true)
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS)
        .depth_bounds_test_enable(false)
        .stencil_test_enable(false);

    PipelineBuilder::new()
        .device(device.device.clone())
        .descriptor_set_layouts(descriptor_set_layouts)
        .multisampling(&multisampling)
        .shader_stages(&shader_stages)
        .vertex_input_info(Vertex::get_input_state_create_info())
        .push_constant_ranges(&push_constant_ranges)
        .viewports(&viewport)
        .scissors(&scissor)
        .rasterizer(&rasterizer)
        .depth_stencil(&depth_stencil)
        .render_pass(render_pass)
        .build()
}

fn make_skybox_pipeline(
    device: &Device,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    descriptor_set_layouts: &[vk::DescriptorSetLayout],
) -> Pipeline {
    let fov = FOV.to_radians();
    let camera_parameters = Vec2::new(
        f32::tan(fov / 2.0),
        (extent.height as f32) / (extent.width as f32)
    );

    let fragment_specialization = SpecializationInfo::new(
        &[
            vk::SpecializationMapEntry {
                constant_id: 0,
                offset: offset_of!(Vec4, x) as u32,
                size: std::mem::size_of::<f32>(),
            },
            vk::SpecializationMapEntry {
                constant_id: 1,
                offset: offset_of!(Vec4, y) as u32,
                size: std::mem::size_of::<f32>(),
            }
        ],
        unsafe {
            std::slice::from_raw_parts(
                (&raw const camera_parameters).cast::<u8>(),
                std::mem::size_of::<Vec2>(),
            )
        },
    );

    let shader_stages = [
        spv!(
            device.device.clone(),
            "skybox.vert",
            vk::ShaderStageFlags::VERTEX,
            None
        ),
        spv!(
            device.device.clone(),
            "skybox.frag",
            vk::ShaderStageFlags::FRAGMENT,
            Some(fragment_specialization)
        ),
    ];

    let viewport = [vk::Viewport::default()
        .x(0.0)
        .y(0.0)
        .width(extent.width as f32)
        .height(extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0)];

    let scissor = [vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent,
    }];

    let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
        .sample_shading_enable(false)
        .rasterization_samples(device.msaa_samples)
        .min_sample_shading(1.0);

    PipelineBuilder::new()
        .device(device.device.clone())
        .shader_stages(&shader_stages)
        .multisampling(&multisampling)
        .descriptor_set_layouts(&descriptor_set_layouts[0..1])
        .viewports(&viewport)
        .scissors(&scissor)
        .render_pass(render_pass)
        .build()
}

const PIPELINES: [for<'a, 'b> fn(
    &'a device::Device,
    vk::Extent2D,
    ash::vk::RenderPass,
    &'b [ash::vk::DescriptorSetLayout],
) -> Pipeline; 2] = [make_main_pipeline, make_skybox_pipeline];

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
            PIPELINES.iter(),
            Some(&self.swapchain),
        );

        self.swapchain = swapchain;
    }

    pub fn new(width: u32, height: u32) -> Self {
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
            PIPELINES.iter(),
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
