use std::{ops::Deref, rc::Rc};

use ash::vk;
use log::info;

use crate::swapchain::Swapchain;

pub struct RenderPass {
    render_pass: vk::RenderPass,
    device: Rc<ash::Device>,
}

impl RenderPass {
    pub fn new(device: Rc<ash::Device>, swapchain: &Swapchain) -> Self {
        let color_attachment = [vk::AttachmentDescription::default()
            .format(swapchain.surface_format.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];

        let color_attachment_ref = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];

        let subpass = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_ref)];

        let render_pass_create_info = vk::RenderPassCreateInfo::default()
            .attachments(&color_attachment)
            .subpasses(&subpass);

        let render_pass = unsafe {
            device
                .create_render_pass(&render_pass_create_info, None)
                .unwrap()
        };

        Self {
            device,
            render_pass,
        }
    }
}

impl Deref for RenderPass {
    type Target = vk::RenderPass;

    fn deref(&self) -> &Self::Target {
        &self.render_pass
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        info!("dropped render pass");
        unsafe {
            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}
