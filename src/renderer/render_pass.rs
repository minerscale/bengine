use std::{ops::Deref, sync::Arc};

use ash::vk;
use log::debug;

use crate::renderer::{device::Device, pipeline::Pipeline, swapchain::find_depth_format};

pub struct RenderPass {
    render_pass: vk::RenderPass,
    pub pipelines: Vec<Pipeline>,
    device: Arc<Device>,
}

impl RenderPass {
    pub fn new<
        T: Iterator<
            Item = impl Fn(
                &Arc<Device>,
                vk::Extent2D,
                vk::RenderPass,
                &[vk::DescriptorSetLayout],
            ) -> Pipeline,
        >,
    >(
        device: &Arc<Device>,
        format: vk::Format,
        extent: vk::Extent2D,
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
        pipelines: T,
    ) -> Self {
        let color_attachment = vk::AttachmentDescription::default()
            .format(format)
            .samples(device.msaa_samples)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(match device.msaa_samples {
                vk::SampleCountFlags::TYPE_1 => vk::ImageLayout::PRESENT_SRC_KHR,
                _ => vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            });

        let depth_attachment = vk::AttachmentDescription::default()
            .format(find_depth_format(&device.instance, device.physical_device))
            .samples(device.msaa_samples)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let attachments = if device.msaa_samples == vk::SampleCountFlags::TYPE_1 {
            vec![color_attachment, depth_attachment]
        } else {
            let color_attachment_resolve = vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

            vec![color_attachment, depth_attachment, color_attachment_resolve]
        };

        let color_attachment_ref = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];

        let depth_attachment_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };

        let color_attachment_resolve_ref = [vk::AttachmentReference {
            attachment: 2,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_ref)
            .depth_stencil_attachment(&depth_attachment_ref);

        let subpass = if device.msaa_samples == vk::SampleCountFlags::TYPE_1 {
            [subpass]
        } else {
            [subpass.resolve_attachments(&color_attachment_resolve_ref)]
        };

        let dependency = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            )];

        let render_pass_create_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(&subpass)
            .dependencies(&dependency);

        let render_pass = unsafe {
            device
                .create_render_pass(&render_pass_create_info, None)
                .unwrap()
        };

        let pipelines = pipelines
            .map(|pipeline| pipeline(device, extent, render_pass, descriptor_set_layouts))
            .collect::<Vec<_>>();

        Self {
            device: device.clone(),
            render_pass,
            pipelines,
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
        debug!("dropped render pass");
        unsafe {
            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}
