use std::sync::Arc;

use ash::vk;
use easy_cast::Cast;
use image::{DynamicImage, GenericImageView};
use log::debug;

use crate::renderer::{
    buffer::{Buffer, find_memory_type},
    command_buffer::ActiveCommandBuffer,
    device::Device,
    render_pass::RenderPass,
};

#[allow(dead_code)]
pub struct SwapchainImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub framebuffer: vk::Framebuffer,
    pub extent: vk::Extent2D,
    device: Arc<Device>,
}

impl SwapchainImage {
    pub fn new(
        device: Arc<Device>,
        image: vk::Image,
        format: vk::Format,
        extent: vk::Extent2D,
        depth_attachment: vk::ImageView,
        color_attachment: Option<vk::ImageView>,
        render_pass: &RenderPass,
    ) -> Self {
        let view = create_image_view(&device, image, format, vk::ImageAspectFlags::COLOR, 1);

        let attachments = color_attachment.map_or_else(
            || vec![view, depth_attachment],
            |color_attachment| vec![color_attachment, depth_attachment, view],
        );

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

    pub mip_levels: u32,

    device: Arc<Device>,
}

fn copy_buffer_to_image<C: ActiveCommandBuffer>(
    device: &ash::Device,
    image: vk::Image,
    extent: vk::Extent2D,
    cmd_buf: &mut C,
    buffer: Arc<Buffer<u8>>,
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

#[derive(Copy, Clone, Debug)]
pub struct ImageCreateInfo {
    pub sample_count: vk::SampleCountFlags,
    pub format: vk::Format,
    pub tiling: vk::ImageTiling,
    pub usage: vk::ImageUsageFlags,
    pub memory_properties: vk::MemoryPropertyFlags,
    pub aspect_flags: vk::ImageAspectFlags,
    pub mipmapping: bool,
}

impl Image {
    pub fn transition_layout<C: ActiveCommandBuffer>(
        &self,
        device: &ash::Device,
        cmd_buf: &mut C,
        mip_level: Option<u32>,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) {
        fn get_access_and_stage_masks(
            layout: vk::ImageLayout,
        ) -> (vk::AccessFlags, vk::PipelineStageFlags) {
            match layout {
                vk::ImageLayout::UNDEFINED => (
                    vk::AccessFlags::empty(),
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                ),
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL => (
                    vk::AccessFlags::TRANSFER_READ,
                    vk::PipelineStageFlags::TRANSFER,
                ),
                vk::ImageLayout::TRANSFER_DST_OPTIMAL => (
                    vk::AccessFlags::TRANSFER_WRITE,
                    vk::PipelineStageFlags::TRANSFER,
                ),
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => (
                    vk::AccessFlags::SHADER_READ,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                ),
                vk::ImageLayout::GENERAL => (
                    vk::AccessFlags::SHADER_WRITE,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                ),
                _ => unimplemented!("unsupported layout {layout:?}"),
            }
        }

        let (src_access_mask, src_stage_mask) = get_access_and_stage_masks(old_layout);
        let (dst_access_mask, dst_stage_mask) = get_access_and_stage_masks(new_layout);

        let barrier = [vk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: mip_level.unwrap_or(0),
                level_count: match mip_level {
                    Some(_) => 1,
                    None => self.mip_levels,
                },
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

    pub fn from_image<C: ActiveCommandBuffer>(
        device: &Arc<Device>,
        cmd_buf: &mut C,
        image: DynamicImage,
        gamma_correction: bool,
    ) -> Arc<Self> {
        let extent = image.dimensions();
        let img = image.into_rgba8().into_vec();

        let info = ImageCreateInfo {
            sample_count: vk::SampleCountFlags::TYPE_1,
            format: if gamma_correction {
                vk::Format::R8G8B8A8_SRGB
            } else {
                vk::Format::R8G8B8A8_UNORM
            },
            tiling: vk::ImageTiling::OPTIMAL,
            usage: vk::ImageUsageFlags::empty(),
            memory_properties: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            aspect_flags: vk::ImageAspectFlags::COLOR,
            mipmapping: true,
        };

        Self::new_staged(
            device,
            vk::Extent2D {
                width: extent.0,
                height: extent.1,
            },
            &img,
            cmd_buf,
            info,
        )
    }

    pub fn from_bytes<C: ActiveCommandBuffer>(
        device: &Arc<Device>,
        cmd_buf: &mut C,
        bytes: &[u8],
    ) -> Arc<Self> {
        let image = ::image::load_from_memory(bytes).unwrap();

        Self::from_image(device, cmd_buf, image, true)
    }

    pub fn generate_mipmaps<C: ActiveCommandBuffer>(&self, device: &Device, cmd_buf: &mut C) {
        let mut mip_width: i32 = self.extent.width.cast();
        let mut mip_height: i32 = self.extent.height.cast();
        for i in 1..self.mip_levels {
            let next_width = (mip_width / 2).max(1);
            let next_height = (mip_height / 2).max(1);

            self.transition_layout(
                device,
                cmd_buf,
                Some(i - 1),
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            );

            let blit = [vk::ImageBlit {
                src_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i - 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                src_offsets: [
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D {
                        x: mip_width,
                        y: mip_height,
                        z: 1,
                    },
                ],
                dst_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                dst_offsets: [
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D {
                        x: next_width,
                        y: next_height,
                        z: 1,
                    },
                ],
            }];

            unsafe {
                device.cmd_blit_image(
                    **cmd_buf,
                    self.image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    self.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &blit,
                    vk::Filter::LINEAR,
                );
            }

            mip_width = next_width;
            mip_height = next_height;
        }

        self.transition_layout(
            device,
            cmd_buf,
            Some(self.mip_levels - 1),
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        );
    }

    fn new_staged<C: ActiveCommandBuffer>(
        device: &Arc<Device>,
        extent: vk::Extent2D,
        image_data: &[u8],
        cmd_buf: &mut C,
        info: ImageCreateInfo,
    ) -> Arc<Self> {
        let mipmapping = info.mipmapping;

        let info = ImageCreateInfo {
            usage: info.usage
                | if mipmapping {
                    vk::ImageUsageFlags::TRANSFER_SRC
                } else {
                    vk::ImageUsageFlags::empty()
                }
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED,
            ..info
        };

        let image = Arc::new(Self::new(device.clone(), extent, info));

        let staging_buffer = Arc::new(Buffer::new(
            device,
            image_data,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        ));

        cmd_buf.add_dependency(image.clone());

        image.transition_layout(
            device,
            cmd_buf,
            None,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );

        copy_buffer_to_image(device, image.image, extent, cmd_buf, staging_buffer);

        if mipmapping {
            image.generate_mipmaps(device, cmd_buf);
        }

        image.transition_layout(
            device,
            cmd_buf,
            None,
            if mipmapping {
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL
            } else {
                vk::ImageLayout::TRANSFER_DST_OPTIMAL
            },
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );

        image
    }

    pub fn new_with_layout<C: ActiveCommandBuffer>(
        device: &Arc<Device>,
        extent: vk::Extent2D,
        info: ImageCreateInfo,
        cmd_buf: &mut C,
        layout: vk::ImageLayout,
    ) -> Self {
        let image = Self::new(device.clone(), extent, info);

        image.transition_layout(device, cmd_buf, None, vk::ImageLayout::UNDEFINED, layout);

        image
    }

    pub fn new(device: Arc<Device>, extent: vk::Extent2D, info: ImageCreateInfo) -> Self {
        let mip_levels = if info.mipmapping {
            extent.width.max(extent.height).ilog2() + 1
        } else {
            1
        };

        let create_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width: extent.width,
                height: extent.height,
                depth: 1,
            })
            .mip_levels(mip_levels)
            .array_layers(1)
            .format(info.format)
            .tiling(info.tiling)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(info.usage)
            .samples(info.sample_count)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let (image, memory) = unsafe {
            let image = device.create_image(&create_info, None).unwrap();
            let memory_requirements = device.get_image_memory_requirements(image);

            let alloc_info = vk::MemoryAllocateInfo::default()
                .allocation_size(memory_requirements.size)
                .memory_type_index(find_memory_type(
                    &device.instance,
                    device.physical_device,
                    memory_requirements.memory_type_bits,
                    info.memory_properties,
                ));

            let memory = device.allocate_memory(&alloc_info, None).unwrap();
            device.bind_image_memory(image, memory, 0).unwrap();

            (image, memory)
        };

        Self {
            image,
            view: create_image_view(&device, image, info.format, info.aspect_flags, mip_levels),
            memory,
            extent,
            device,
            mip_levels,
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        debug!("dropped image view");
        unsafe { self.device.destroy_image_view(self.view, None) };

        debug!("dropped image");
        unsafe {
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

impl Drop for SwapchainImage {
    fn drop(&mut self) {
        debug!("dropped framebuffer");
        unsafe { self.device.destroy_framebuffer(self.framebuffer, None) };

        debug!("dropped image view");
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
    mip_levels: u32,
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
            level_count: mip_levels,
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
