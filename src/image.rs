use std::rc::Rc;

use ash::{khr, vk};
use log::info;

use crate::pipeline::Pipeline;

pub struct Image {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub framebuffer: Option<vk::Framebuffer>,
    pub extent: vk::Extent2D,

    device: Rc<ash::Device>,
}

impl Image {
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

    pub fn new(
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

impl Drop for Image {
    fn drop(&mut self) {
        if let Some(framebuffer) = self.framebuffer {
            info!("dropped framebuffer");
            unsafe { self.device.destroy_framebuffer(framebuffer, None) };
        }

        info!("dropped image view");
        unsafe { self.device.destroy_image_view(self.view, None) };
    }
}
