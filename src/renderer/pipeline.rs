use std::{mem::offset_of, ops::Deref, rc::Rc};

use ash::vk;
use log::info;
use ultraviolet::{Isometry3, Vec4};

#[repr(C)]
pub struct VertexPushConstants {
    pub model_transform: Isometry3,
}

use crate::renderer::{device::Device, shader_module::spv, vertex::Vertex};

pub struct Pipeline {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    device: Rc<ash::Device>,
}

impl Pipeline {
    pub fn new(
        device: &Device,
        extent: &vk::Extent2D,
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
        render_pass: vk::RenderPass,
    ) -> Self {
        let vert_shader_module = spv!(device.device.clone(), "shader.vert");
        let frag_shader_module = spv!(device.device.clone(), "shader.frag");

        let fov = 90f32.to_radians();

        let ez = f32::tan(fov / 2.0).recip();
        let camera_parameters = Vec4::new(
            ez,
            -((extent.width as f32) / (extent.height as f32)),
            0.01,
            1000.0,
        );

        let specialization_map_entries = [
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
        ];

        let specialization_info = vk::SpecializationInfo::default()
            .map_entries(&specialization_map_entries)
            .data(unsafe {
                std::slice::from_raw_parts(
                    &camera_parameters as *const Vec4 as *const u8,
                    std::mem::size_of::<Vec4>(),
                )
            });

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(*vert_shader_module)
                .name(c"main")
                .specialization_info(&specialization_info),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(*frag_shader_module)
                .name(c"main"),
        ];

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let vertex_binding_descriptions = [Vertex::get_binding_description()];
        let vertex_attribute_descriptions = Vertex::get_attribute_descriptions();
        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_binding_descriptions)
            .vertex_attribute_descriptions(&vertex_attribute_descriptions);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let viewport = [vk::Viewport::default()
            .x(0.0)
            .y(0.0)
            .width(extent.width as f32)
            .height(extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0)];

        let scissor = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D {
                width: extent.width,
                height: extent.height,
            },
        }];

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewport)
            .scissors(&scissor);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .sample_shading_enable(false)
            .rasterization_samples(device.mssa_samples)
            .min_sample_shading(1.0);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        let color_blend_attachment = [vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::TRUE,
            src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_alpha_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::RGBA,
        }];

        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(&color_blend_attachment);

        let push_constant_ranges = [vk::PushConstantRange::default()
            .offset(0)
            .size(
                std::mem::size_of::<VertexPushConstants>()
                    .try_into()
                    .unwrap(),
            )
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)];

        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(descriptor_set_layouts)
            .push_constant_ranges(&push_constant_ranges);

        let pipeline_layout = unsafe {
            device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .unwrap()
        };

        let pipeline_info = [vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blending)
            .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0)];

        let pipeline = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
                .expect("failed to create graphics pipeline!")[0]
        };

        Self {
            device: device.device.clone(),
            pipeline,
            pipeline_layout,
        }
    }
}

impl Deref for Pipeline {
    type Target = vk::Pipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        info!("dropped pipeline");
        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}
