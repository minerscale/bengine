use std::{ops::Deref, rc::Rc};

use ash::vk;
use log::info;
use ultraviolet::Isometry3;

#[repr(C)]
pub struct VertexPushConstants {
    pub model_transform: Isometry3,
}

use crate::renderer::{shader_module::ShaderModule, vertex::Vertex};

pub struct Pipeline {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    device: Rc<ash::Device>,
}

#[derive(Default)]
pub struct PipelineBuilder<'a> {
    device: Option<Rc<ash::Device>>,
    extent: Option<vk::Extent2D>,
    descriptor_set_layouts: Option<&'a [vk::DescriptorSetLayout]>,
    render_pass: Option<vk::RenderPass>,
    msaa_samples: Option<vk::SampleCountFlags>,
    shader_stages: Option<&'a [ShaderModule<'a>]>,
}

impl<'a> PipelineBuilder<'a> {
    pub fn new() -> Self {
        PipelineBuilder::default()
    }

    pub fn device(self, device: Rc<ash::Device>) -> Self {
        PipelineBuilder {
            device: Some(device),
            ..self
        }
    }

    pub fn extent(self, extent: vk::Extent2D) -> Self {
        PipelineBuilder {
            extent: Some(extent),
            ..self
        }
    }

    pub fn descriptor_set_layouts(
        self,
        descriptor_set_layouts: &'a [vk::DescriptorSetLayout],
    ) -> Self {
        PipelineBuilder {
            descriptor_set_layouts: Some(descriptor_set_layouts),
            ..self
        }
    }

    pub fn render_pass(self, render_pass: vk::RenderPass) -> Self {
        PipelineBuilder {
            render_pass: Some(render_pass),
            ..self
        }
    }

    pub fn msaa_samples(self, msaa_samples: vk::SampleCountFlags) -> Self {
        PipelineBuilder {
            msaa_samples: Some(msaa_samples),
            ..self
        }
    }

    pub fn shader_stages(self, shader_stages: &'a [ShaderModule<'a>]) -> Self {
        PipelineBuilder {
            shader_stages: Some(shader_stages),
            ..self
        }
    }

    pub fn build(self) -> Pipeline {
        let device = self
            .device
            .expect("pipeline build error: device is required");

        let extent = self
            .extent
            .expect("pipeline build error: extent is required");

        fn make_stage_info<'a>(
            shader_module: &'a ShaderModule<'a>,
        ) -> vk::PipelineShaderStageCreateInfo<'a> {
            let mut stage_info = vk::PipelineShaderStageCreateInfo::default()
                .stage(shader_module.stage)
                .module(**shader_module)
                .name(c"main");

            if let Some(info) = shader_module.specialization_info.as_ref() {
                stage_info = stage_info.specialization_info(info)
            }

            stage_info
        }

        let shader_stages = self
            .shader_stages
            .expect("pipeline build error: shader_stages required")
            .iter()
            .map(make_stage_info)
            .collect::<Vec<_>>();

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
            .rasterization_samples(
                self.msaa_samples
                    .expect("pipeline build error: msaa_samples required"),
            )
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
            .set_layouts(
                self.descriptor_set_layouts
                    .expect("pipeline build error: descriptor_set_layouts required"),
            )
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
            .render_pass(
                self.render_pass
                    .expect("pipeline build error: render_pass required"),
            )
            .subpass(0)];

        let pipeline = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
                .expect("failed to create graphics pipeline!")[0]
        };

        Pipeline {
            device: device.clone(),
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
