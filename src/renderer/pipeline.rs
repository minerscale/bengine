use std::{ops::Deref, rc::Rc};

use ash::vk;
use log::info;
use ultraviolet::Isometry3;

#[repr(C)]
pub struct VertexPushConstants {
    pub model_transform: Isometry3,
}

use crate::renderer::shader_module::ShaderModule;

pub struct Pipeline {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    device: Rc<ash::Device>,
}

#[derive(Default)]
pub struct PipelineBuilder<'a> {
    device: Option<Rc<ash::Device>>,
    descriptor_set_layouts: Option<&'a [vk::DescriptorSetLayout]>,
    render_pass: Option<vk::RenderPass>,
    multisampling: Option<&'a vk::PipelineMultisampleStateCreateInfo<'a>>,
    shader_stages: Option<&'a [ShaderModule<'a>]>,
    dynamic_states: Option<&'a [vk::DynamicState]>,
    vertex_input_info: Option<&'a vk::PipelineVertexInputStateCreateInfo<'a>>,
    push_constant_ranges: Option<&'a [vk::PushConstantRange]>,
    input_assembly: Option<&'a vk::PipelineInputAssemblyStateCreateInfo<'a>>,
    viewports: Option<&'a [vk::Viewport]>,
    scissors: Option<&'a [vk::Rect2D]>,
    rasterizer: Option<&'a vk::PipelineRasterizationStateCreateInfo<'a>>,
    depth_stencil: Option<&'a vk::PipelineDepthStencilStateCreateInfo<'a>>,
    color_blending: Option<&'a vk::PipelineColorBlendStateCreateInfo<'a>>,
}

impl<'a> PipelineBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn device(self, device: Rc<ash::Device>) -> Self {
        Self {
            device: Some(device),
            ..self
        }
    }

    pub fn descriptor_set_layouts(
        self,
        descriptor_set_layouts: &'a [vk::DescriptorSetLayout],
    ) -> Self {
        Self {
            descriptor_set_layouts: Some(descriptor_set_layouts),
            ..self
        }
    }

    pub fn render_pass(self, render_pass: vk::RenderPass) -> Self {
        Self {
            render_pass: Some(render_pass),
            ..self
        }
    }

    pub fn multisampling(self, multisampling: &'a vk::PipelineMultisampleStateCreateInfo) -> Self {
        Self {
            multisampling: Some(multisampling),
            ..self
        }
    }

    pub fn shader_stages(self, shader_stages: &'a [ShaderModule<'a>]) -> Self {
        Self {
            shader_stages: Some(shader_stages),
            ..self
        }
    }

    pub fn dynamic_states(self, dynamic_states: &'a [vk::DynamicState]) -> Self {
        Self {
            dynamic_states: Some(dynamic_states),
            ..self
        }
    }

    pub fn vertex_input_info(
        self,
        vertex_input_info: &'a vk::PipelineVertexInputStateCreateInfo<'a>,
    ) -> Self {
        Self {
            vertex_input_info: Some(vertex_input_info),
            ..self
        }
    }

    pub fn push_constant_ranges(self, push_constant_ranges: &'a [vk::PushConstantRange]) -> Self {
        Self {
            push_constant_ranges: Some(push_constant_ranges),
            ..self
        }
    }

    pub fn viewports(self, viewports: &'a [vk::Viewport]) -> Self {
        Self {
            viewports: Some(viewports),
            ..self
        }
    }

    pub fn scissors(self, scissors: &'a [vk::Rect2D]) -> Self {
        Self {
            scissors: Some(scissors),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn input_assembly(
        self,
        input_assembly: &'a vk::PipelineInputAssemblyStateCreateInfo<'a>,
    ) -> Self {
        Self {
            input_assembly: Some(input_assembly),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn rasterizer(self, rasterizer: &'a vk::PipelineRasterizationStateCreateInfo<'a>) -> Self {
        Self {
            rasterizer: Some(rasterizer),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn depth_stencil(
        self,
        depth_stencil: &'a vk::PipelineDepthStencilStateCreateInfo<'a>,
    ) -> Self {
        Self {
            depth_stencil: Some(depth_stencil),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn color_blending(
        self,
        color_blending: &'a vk::PipelineColorBlendStateCreateInfo<'a>,
    ) -> Self {
        Self {
            color_blending: Some(color_blending),
            ..self
        }
    }

    pub fn build(&self) -> Pipeline {
        let device = self
            .device.as_ref()
            .expect("pipeline build error: device is required");

        let shader_stages = self
            .shader_stages
            .expect("pipeline build error: shader_stages required")
            .iter()
            .map(ShaderModule::stage_info)
            .collect::<Vec<_>>();

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(
                self.viewports
                    .expect("pipeline build error: viewport required"),
            )
            .scissors(
                self.scissors
                    .expect("pipeline build error: scissor required"),
            );

        let mut pipeline_layout_info = vk::PipelineLayoutCreateInfo::default().set_layouts(
            self.descriptor_set_layouts
                .expect("pipeline build error: descriptor_set_layouts required"),
        );

        if let Some(push_constant_ranges) = self.push_constant_ranges {
            pipeline_layout_info = pipeline_layout_info.push_constant_ranges(push_constant_ranges);
        }

        let pipeline_layout = unsafe {
            device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .unwrap()
        };

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false);

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

        let mut pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(
                self.vertex_input_info
                    .expect("pipeline build error: vertex_input_info required"),
            )
            .input_assembly_state(
                self.input_assembly.unwrap_or(&input_assembly)
            )
            .viewport_state(&viewport_state)
            .rasterization_state(
                self.rasterizer.unwrap_or(&rasterizer),
            )
            .multisample_state(
                self.multisampling
                    .expect("pipeline build error: multisampling required"),
            )
            .depth_stencil_state(
                self.depth_stencil
                    .unwrap_or(&depth_stencil),
            )
            .color_blend_state(
                self.color_blending
                    .unwrap_or(&color_blending),
            )
            .layout(pipeline_layout)
            .render_pass(
                self.render_pass
                    .expect("pipeline build error: render_pass required"),
            )
            .subpass(0);

        let dynamic_state;
        if let Some(dynamic_states) = self.dynamic_states.as_ref() {
            dynamic_state =
                vk::PipelineDynamicStateCreateInfo::default().dynamic_states(dynamic_states);

            pipeline_info = pipeline_info.dynamic_state(&dynamic_state);
        }

        let pipeline = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
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
