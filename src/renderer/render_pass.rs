use std::{mem::offset_of, ops::Deref, rc::Rc};

use ash::vk;
use log::info;
use ultraviolet::Vec4;

use crate::renderer::{
    device::Device,
    pipeline::{Pipeline, PipelineBuilder},
    swapchain::find_depth_format,
};

use super::shader_module::{SpecializationInfo, spv};

pub struct RenderPass {
    render_pass: vk::RenderPass,
    pub pipeline: Pipeline,
    device: Rc<ash::Device>,
}

impl RenderPass {
    pub fn new(
        instance: &ash::Instance,
        device: &Device,
        format: vk::Format,
        extent: vk::Extent2D,
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
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
            .format(find_depth_format(instance, &device.physical_device))
            .samples(device.msaa_samples)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let attachments = match device.msaa_samples {
            vk::SampleCountFlags::TYPE_1 => vec![color_attachment, depth_attachment],
            _ => {
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
            }
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

        let subpass = if device.msaa_samples != vk::SampleCountFlags::TYPE_1 {
            [subpass.resolve_attachments(&color_attachment_resolve_ref)]
        } else {
            [subpass]
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

        let fov = 90f32.to_radians();
        let ez = f32::tan(fov / 2.0).recip();
        let camera_parameters = Vec4::new(
            ez,
            -((extent.width as f32) / (extent.height as f32)),
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
                    &camera_parameters as *const Vec4 as *const u8,
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

        let pipeline = PipelineBuilder::new()
            .device(device.device.clone())
            .extent(extent)
            .descriptor_set_layouts(descriptor_set_layouts)
            .render_pass(render_pass)
            .msaa_samples(device.msaa_samples)
            .shader_stages(&shader_stages)
            .build();

        Self {
            device: device.device.clone(),
            render_pass,
            pipeline,
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
