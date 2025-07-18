use std::{mem::ManuallyDrop, ops::Deref};

use ash::{khr, vk};
use log::{debug, info, warn};

use crate::renderer::{
    device::Device,
    image::{Image, SwapchainImage, find_supported_format},
    pipeline::Pipeline,
    render_pass::RenderPass,
};

pub struct Swapchain {
    pub loader: khr::swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub render_pass: RenderPass,
    pub images: Vec<SwapchainImage>,
    pub depth_image: ManuallyDrop<Image>,
    pub color_image: Option<Image>,
}

impl Swapchain {
    pub fn new<
        'a,
        T: Iterator<
            Item = &'a (
                           impl Fn(
                &Device,
                vk::Extent2D,
                vk::RenderPass,
                &[vk::DescriptorSetLayout],
            ) -> Pipeline
                           + 'a
                       ),
        >,
    >(
        instance: &ash::Instance,
        device: &Device,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
        extent: vk::Extent2D,
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
        pipelines: T,
        old_swapchain: Option<&Self>,
    ) -> Self {
        info!("creating new swapchain");

        let swapchain_loader = old_swapchain.map_or_else(
            || khr::swapchain::Device::new(instance, device),
            |swapchain| swapchain.loader.clone(),
        );

        let surface_format =
            Self::choose_swap_surface_format(device.physical_device, surface_loader, surface);

        let surface_capabilities = unsafe {
            surface_loader
                .get_physical_device_surface_capabilities(device.physical_device, surface)
                .unwrap()
        };

        let mut desired_image_count = surface_capabilities.min_image_count + 1;
        if surface_capabilities.max_image_count > 0
            && desired_image_count > surface_capabilities.max_image_count
        {
            desired_image_count = surface_capabilities.max_image_count;
        }

        let ub = surface_capabilities.max_image_extent;
        let lb = surface_capabilities.min_image_extent;

        let width = u32::clamp(extent.width, lb.width, ub.width);
        let height = u32::clamp(extent.height, lb.height, ub.height);
        let extent = vk::Extent2D { width, height };

        let pre_transform = if surface_capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_capabilities.current_transform
        };
        let present_modes = unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(device.physical_device, surface)
                .unwrap()
        };
        let present_mode = present_modes
            .into_iter()
            .find(|&mode| mode == vk::PresentModeKHR::FIFO_RELAXED)
            .unwrap_or(vk::PresentModeKHR::FIFO);

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(desired_image_count)
            .image_color_space(surface_format.color_space)
            .image_format(surface_format.format)
            .image_extent(extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .image_array_layers(1)
            .old_swapchain(old_swapchain.map_or_else(vk::SwapchainKHR::null, |s| s.swapchain));

        let swapchain = unsafe {
            swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap()
        };

        let depth_image = {
            let depth_format = find_depth_format(instance, device.physical_device);

            fn has_stencil_component(format: vk::Format) -> bool {
                format == vk::Format::D32_SFLOAT_S8_UINT || format == vk::Format::D24_UNORM_S8_UINT
            }

            has_stencil_component(depth_format);

            ManuallyDrop::new(Image::new(
                instance,
                device.physical_device,
                device.device.clone(),
                extent,
                device.msaa_samples,
                depth_format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                vk::ImageAspectFlags::DEPTH,
                false,
            ))
        };

        let color_image = match device.msaa_samples {
            vk::SampleCountFlags::TYPE_1 => None,
            _ => Some(Image::new(
                instance,
                device.physical_device,
                device.device.clone(),
                extent,
                device.msaa_samples,
                surface_format.format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::TRANSIENT_ATTACHMENT | vk::ImageUsageFlags::COLOR_ATTACHMENT,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                vk::ImageAspectFlags::COLOR,
                false,
            )),
        };

        let render_pass = RenderPass::new(
            instance,
            device,
            surface_format.format,
            extent,
            descriptor_set_layouts,
            pipelines,
        );

        let images = unsafe { swapchain_loader.get_swapchain_images(swapchain).unwrap() }
            .iter()
            .map(|&image| {
                SwapchainImage::new(
                    device.device.clone(),
                    image,
                    surface_format.format,
                    extent,
                    depth_image.view,
                    color_image.as_ref().map(|i| i.view),
                    &render_pass,
                )
            })
            .collect::<Vec<_>>();

        Self {
            loader: swapchain_loader,
            swapchain,
            render_pass,
            images,
            depth_image,
            color_image,
        }
    }

    fn choose_swap_surface_format(
        physical_device: vk::PhysicalDevice,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> vk::SurfaceFormatKHR {
        let avaliable_formats = unsafe {
            surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .unwrap()
        };

        avaliable_formats
            .iter()
            .find_map(|&available_format| {
                matches!(
                    available_format,
                    vk::SurfaceFormatKHR {
                        format: vk::Format::B8G8R8A8_SRGB,
                        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR
                    }
                )
                .then_some(available_format)
            })
            .unwrap_or_else(|| {
                let format = avaliable_formats.first().unwrap();
                warn!("couldn't find suitable format! Falling back to {format:?}");
                *format
            })
    }
}

impl Deref for Swapchain {
    type Target = vk::SwapchainKHR;

    fn deref(&self) -> &Self::Target {
        &self.swapchain
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            // Drop images first
            self.images.clear();
            ManuallyDrop::drop(&mut self.depth_image);
            self.color_image.take();

            debug!("dropped swapchain");
            self.loader.destroy_swapchain(self.swapchain, None);
        }
    }
}

pub fn find_depth_format(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> vk::Format {
    find_supported_format(
        instance,
        physical_device,
        &[vk::Format::D32_SFLOAT, vk::Format::D32_SFLOAT_S8_UINT],
        vk::ImageTiling::OPTIMAL,
        vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
    )
}
