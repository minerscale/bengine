use std::rc::Rc;

use ash::vk;
use log::info;

use crate::{buffer::find_memory_type, pipeline::Pipeline};

pub struct SwapchainImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub framebuffer: vk::Framebuffer,
    pub extent: vk::Extent2D,
    device: Rc<ash::Device>,
}

impl SwapchainImage {
    pub fn new(
        device: Rc<ash::Device>,
        image: vk::Image,
        format: vk::Format,
        extent: vk::Extent2D,
        attachment: vk::ImageView,
        pipeline: &Pipeline
    ) -> Self {
        let view = create_image_view(&device, image, format, vk::ImageAspectFlags::COLOR);

        let framebuffer = create_framebuffer(
            &device,
            pipeline,
            &[view, attachment],
            extent,
        );

        SwapchainImage {
            image,
            view,
            framebuffer,
            extent,
            device,
        }
    }
}

pub struct Image {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub memory: vk::DeviceMemory,
    pub extent: vk::Extent2D,

    device: Rc<ash::Device>,
}

impl Image {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instance: &ash::Instance,
        physical_device: &vk::PhysicalDevice,
        device: Rc<ash::Device>,
        extent: vk::Extent2D,
        format: vk::Format,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
        properties: vk::MemoryPropertyFlags,
        aspect_flags: vk::ImageAspectFlags,
    ) -> Self {
        let create_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width: extent.width,
                height: extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(tiling)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(usage)
            .samples(vk::SampleCountFlags::TYPE_1)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let (image, memory) = unsafe {
            let image = device.create_image(&create_info, None).unwrap();
            let memory_requirements = device.get_image_memory_requirements(image);

            let alloc_info = vk::MemoryAllocateInfo::default()
                .allocation_size(memory_requirements.size)
                .memory_type_index(find_memory_type(
                    instance,
                    *physical_device,
                    memory_requirements.memory_type_bits,
                    properties,
                ));

            let memory = device.allocate_memory(&alloc_info, None).unwrap();
            device.bind_image_memory(image, memory, 0).unwrap();

            (image, memory)
        };

        Self {
            image,
            view: create_image_view(&device, image, format, aspect_flags),
            memory,
            extent,
            device,
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        info!("dropped image view");
        unsafe { self.device.destroy_image_view(self.view, None) };

        info!("dropped image");
        unsafe {
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None)
        };
    }
}

impl Drop for SwapchainImage {
    fn drop(&mut self) {
        info!("dropped framebuffer");
        unsafe { self.device.destroy_framebuffer(self.framebuffer, None) };

        info!("dropped image view");
        unsafe { self.device.destroy_image_view(self.view, None) };
    }
}

pub fn find_supported_format(
    instance: &ash::Instance,
    physical_device: &vk::PhysicalDevice,
    candidates: Vec<vk::Format>,
    tiling: vk::ImageTiling,
    features: vk::FormatFeatureFlags,
) -> vk::Format {
    *candidates
        .iter()
        .find(|&&format| {
            let properties =
                unsafe { instance.get_physical_device_format_properties(*physical_device, format) };

            (tiling == vk::ImageTiling::LINEAR
                && properties.linear_tiling_features.contains(features))
                || (tiling == vk::ImageTiling::OPTIMAL
                    && properties.optimal_tiling_features.contains(features))
        })
        .expect("failed to find supported format!")
}

pub fn create_image_view(
    device: &ash::Device,
    image: vk::Image,
    format: vk::Format,
    aspect_flags: vk::ImageAspectFlags,
) -> vk::ImageView {
    let create_view_info = vk::ImageViewCreateInfo::default()
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .components(vk::ComponentMapping {
            r: vk::ComponentSwizzle::R,
            g: vk::ComponentSwizzle::G,
            b: vk::ComponentSwizzle::B,
            a: vk::ComponentSwizzle::A,
        })
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: aspect_flags,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })
        .image(image);

    unsafe { device.create_image_view(&create_view_info, None).unwrap() }
}

fn create_framebuffer(
    device: &ash::Device,
    pipeline: &Pipeline,
    attachments: &[vk::ImageView],
    extent: vk::Extent2D,
) -> vk::Framebuffer {
    let framebuffer_info = vk::FramebufferCreateInfo::default()
        .render_pass(*pipeline.render_pass)
        .attachments(attachments)
        .width(extent.width)
        .height(extent.height)
        .layers(1);

    unsafe { device.create_framebuffer(&framebuffer_info, None).unwrap() }
}
