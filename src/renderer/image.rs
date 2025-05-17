use std::rc::Rc;

use ash::vk;
use image::GenericImageView;
use log::info;

use crate::renderer::{
    buffer::{Buffer, find_memory_type},
    command_buffer::ActiveCommandBuffer,
    render_pass::RenderPass,
};

#[allow(dead_code)]
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
        depth_attachment: vk::ImageView,
        color_attachment: Option<vk::ImageView>,
        render_pass: &RenderPass,
    ) -> Self {
        let view = create_image_view(&device, image, format, vk::ImageAspectFlags::COLOR);

        let attachments = color_attachment.map_or_else(|| vec![view, depth_attachment], |color_attachment| vec![color_attachment, depth_attachment, view]);

        let framebuffer = create_framebuffer(&device, render_pass, &attachments, extent);

        Self {
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

fn copy_buffer_to_image<C: ActiveCommandBuffer>(
    device: &ash::Device,
    image: vk::Image,
    extent: vk::Extent2D,
    cmd_buf: &mut C,
    buffer: Rc<Buffer<u8>>,
) {
    let regions = [vk::BufferImageCopy {
        buffer_offset: 0,
        buffer_row_length: 0,
        buffer_image_height: 0,
        image_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
        image_extent: vk::Extent3D {
            width: extent.width,
            height: extent.height,
            depth: 1,
        },
    }];

    unsafe {
        device.cmd_copy_buffer_to_image(
            **cmd_buf,
            **buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &regions,
        );
        cmd_buf.add_dependency(buffer);
    }
}

fn transition_layout<C: ActiveCommandBuffer>(
    device: &ash::Device,
    image: vk::Image,
    cmd_buf: &mut C,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    let (src_access_mask, dst_access_mask, src_stage_mask, dst_stage_mask) =
        match (old_layout, new_layout) {
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
                vk::AccessFlags::empty(),
                vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
            ),
            (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            ),
            _ => {
                unimplemented!("unsupported layout transition")
            }
        };

    let barrier = [vk::ImageMemoryBarrier::default()
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })
        .src_access_mask(src_access_mask)
        .dst_access_mask(dst_access_mask)];

    unsafe {
        device.cmd_pipeline_barrier(
            **cmd_buf,
            src_stage_mask,
            dst_stage_mask,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &barrier,
        );
    }
}

impl Image {
    pub fn from_bytes<C: ActiveCommandBuffer>(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &Rc<ash::Device>,
        cmd_buf: &mut C,
        bytes: &[u8],
    ) -> Self {
        let image = ::image::load_from_memory(bytes).unwrap();
        let extent = image.dimensions();
        let img = image.into_rgba8().into_vec();

        Self::new_staged(
            instance,
            physical_device,
            device,
            vk::Extent2D {
                width: extent.0,
                height: extent.1,
            },
            &img,
            cmd_buf,
            vk::SampleCountFlags::TYPE_1,
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageTiling::OPTIMAL,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            vk::ImageAspectFlags::COLOR,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_staged<C: ActiveCommandBuffer>(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &Rc<ash::Device>,
        extent: vk::Extent2D,
        image_data: &[u8],
        cmd_buf: &mut C,
        sample_count: vk::SampleCountFlags,
        format: vk::Format,
        tiling: vk::ImageTiling,
        properties: vk::MemoryPropertyFlags,
        aspect_flags: vk::ImageAspectFlags,
    ) -> Self {
        let image = Self::new(
            instance,
            physical_device,
            device.clone(),
            extent,
            sample_count,
            format,
            tiling,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            properties,
            aspect_flags,
        );

        let staging_buffer = Rc::new(Buffer::new(
            device.clone(),
            instance,
            physical_device,
            image_data,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        ));

        transition_layout(
            device,
            image.image,
            cmd_buf,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );

        copy_buffer_to_image(device, image.image, extent, cmd_buf, staging_buffer);

        transition_layout(
            device,
            image.image,
            cmd_buf,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );

        image
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: Rc<ash::Device>,
        extent: vk::Extent2D,
        sample_count: vk::SampleCountFlags,
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
            .samples(sample_count)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let (image, memory) = unsafe {
            let image = device.create_image(&create_info, None).unwrap();
            let memory_requirements = device.get_image_memory_requirements(image);

            let alloc_info = vk::MemoryAllocateInfo::default()
                .allocation_size(memory_requirements.size)
                .memory_type_index(find_memory_type(
                    instance,
                    physical_device,
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
            self.device.free_memory(self.memory, None);
        }
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
    physical_device: vk::PhysicalDevice,
    candidates: &[vk::Format],
    tiling: vk::ImageTiling,
    features: vk::FormatFeatureFlags,
) -> vk::Format {
    *candidates
        .iter()
        .find(|&&format| {
            let properties =
                unsafe { instance.get_physical_device_format_properties(physical_device, format) };

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
    render_pass: &RenderPass,
    attachments: &[vk::ImageView],
    extent: vk::Extent2D,
) -> vk::Framebuffer {
    let framebuffer_info = vk::FramebufferCreateInfo::default()
        .render_pass(**render_pass)
        .attachments(attachments)
        .width(extent.width)
        .height(extent.height)
        .layers(1);

    unsafe { device.create_framebuffer(&framebuffer_info, None).unwrap() }
}
