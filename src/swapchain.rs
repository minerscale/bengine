use std::{ops::Deref, rc::Rc};

use ash::{khr, vk};
use log::info;

use crate::{device::Device, pipeline::Pipeline};

pub struct SwapchainImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub framebuffer: Option<vk::Framebuffer>,
    pub extent: vk::Extent2D,

    device: Rc<ash::Device>,
}

impl SwapchainImage {
    pub fn create_framebuffer(&mut self, pipeline: &Pipeline) {
        let attachments = [self.view];

        let framebuffer_info = vk::FramebufferCreateInfo::default()
            .render_pass(*pipeline.render_pass)
            .attachments(&attachments)
            .width(self.extent.width)
            .height(self.extent.height)
            .layers(1);

        self.framebuffer = unsafe {
            Some(
                self.device
                    .create_framebuffer(&framebuffer_info, None)
                    .unwrap(),
            )
        }
    }

    fn new(
        device: Rc<ash::Device>,
        swapchain_loader: &khr::swapchain::Device,
        swapchain: vk::SwapchainKHR,
        surface_format: vk::SurfaceFormatKHR,
        extent: vk::Extent2D,
    ) -> Vec<Self> {
        let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain).unwrap() };

        swapchain_images
            .iter()
            .map(|&image| {
                let create_view_info = vk::ImageViewCreateInfo::default()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(surface_format.format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::R,
                        g: vk::ComponentSwizzle::G,
                        b: vk::ComponentSwizzle::B,
                        a: vk::ComponentSwizzle::A,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image(image);
                Self {
                    device: device.clone(),
                    image,
                    view: unsafe { device.create_image_view(&create_view_info, None).unwrap() },
                    framebuffer: None,
                    extent,
                }
            })
            .collect()
    }
}

impl Drop for SwapchainImage {
    fn drop(&mut self) {
        if let Some(framebuffer) = self.framebuffer {
            info!("dropped framebuffer");
            unsafe { self.device.destroy_framebuffer(framebuffer, None) };
        }

        info!("dropped image view");
        unsafe { self.device.destroy_image_view(self.view, None) };
    }
}

pub struct Swapchain {
    pub device: khr::swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub images: Vec<SwapchainImage>,
    pub surface_format: vk::SurfaceFormatKHR,
    pub extent: vk::Extent2D,
}

impl Swapchain {
    pub fn attach_framebuffers(&mut self, pipeline: &Pipeline) {
        for image in &mut self.images {
            image.create_framebuffer(&pipeline);
        }
    }

    pub fn new(
        instance: &ash::Instance,
        device: &Device,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
        extent: vk::Extent2D,
        old_swapchain: Option<vk::SwapchainKHR>,
    ) -> Self {
        let swapchain_loader = khr::swapchain::Device::new(&instance, &device);

        let surface_format =
            Self::choose_swap_surface_format(device.physical_device, &surface_loader, surface);

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
            .iter()
            .cloned()
            .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO);

        let extent = vk::Extent2D { width, height };
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
            .old_swapchain(old_swapchain.unwrap_or(vk::SwapchainKHR::null()));

        let swapchain = unsafe {
            swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap()
        };

        let images = SwapchainImage::new(
            device.device.clone(),
            &swapchain_loader,
            swapchain,
            surface_format,
            extent,
        );

        Self {
            device: swapchain_loader.clone(),
            swapchain,
            images,
            surface_format,
            extent,
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
                (available_format
                    == (vk::SurfaceFormatKHR {
                        format: vk::Format::B8G8R8A8_SRGB,
                        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
                    }))
                .then_some(available_format)
            })
            .unwrap_or(avaliable_formats[0])
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
            self.images.clear(); // Drop images first

            info!("dropped swapchain");
            self.device.destroy_swapchain(self.swapchain, None)
        }
    }
}
