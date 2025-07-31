use std::{mem::offset_of, sync::Arc};

use ash::vk;
use ultraviolet::{Isometry3, Vec2};

#[repr(C)]
pub struct PushConstants {
    pub model_transform: Isometry3,
    pub material_properties: MaterialProperties,
}

use crate::{
    egui_backend,
    renderer::{
        DescriptorSetLayoutFunction, PipelineFunction,
        descriptors::DescriptorSetLayout,
        device::Device,
        material::MaterialProperties,
        pipeline::{Pipeline, PipelineBuilder},
        shader_module::{SpecializationInfo, spv},
    },
    skybox,
    vertex::Vertex,
};

fn make_main_pipeline(
    device: &Arc<Device>,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    descriptor_set_layouts: &[vk::DescriptorSetLayout],
) -> Pipeline {
    let camera_parameters = Vec2::new(0.01, 1000.0);

    let vertex_specialization = SpecializationInfo::new(
        &[
            vk::SpecializationMapEntry {
                constant_id: 0,
                offset: offset_of!(Vec2, x) as u32,
                size: std::mem::size_of::<f32>(),
            },
            vk::SpecializationMapEntry {
                constant_id: 1,
                offset: offset_of!(Vec2, y) as u32,
                size: std::mem::size_of::<f32>(),
            },
        ],
        unsafe {
            std::slice::from_raw_parts(
                (&raw const camera_parameters).cast::<u8>(),
                std::mem::size_of::<Vec2>(),
            )
        },
    );

    let shader_stages = [
        spv!(
            device.clone(),
            "main.vert",
            vk::ShaderStageFlags::VERTEX,
            Some(vertex_specialization)
        ),
        spv!(
            device.clone(),
            "main.frag",
            vk::ShaderStageFlags::FRAGMENT,
            None
        ),
    ];

    let push_constant_ranges = [vk::PushConstantRange::default()
        .offset(0)
        .size(std::mem::size_of::<PushConstants>().try_into().unwrap())
        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)];

    let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
        .sample_shading_enable(false)
        .rasterization_samples(device.msaa_samples)
        .min_sample_shading(1.0);

    let viewport = [vk::Viewport::default()
        .x(0.0)
        .y(0.0)
        .width(extent.width as f32)
        .height(extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0)];

    let scissor = [vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent,
    }];

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

    PipelineBuilder::new()
        .device(device.clone())
        .descriptor_set_layouts(descriptor_set_layouts)
        .multisampling(&multisampling)
        .shader_stages(&shader_stages)
        .vertex_input_info(Vertex::get_input_state_create_info())
        .push_constant_ranges(&push_constant_ranges)
        .viewports(&viewport)
        .scissors(&scissor)
        .rasterizer(&rasterizer)
        .depth_stencil(&depth_stencil)
        .render_pass(render_pass)
        .build()
}

pub const UNIFORM_BUFFER_LAYOUT: usize = 0;
pub const MATERIAL_LAYOUT: usize = 1;

pub const DESCRIPTOR_SET_LAYOUTS: [DescriptorSetLayoutFunction; 2] = [
    |device: Arc<Device>| {
        DescriptorSetLayout::new(
            device,
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(
                    vk::ShaderStageFlags::VERTEX
                        | vk::ShaderStageFlags::FRAGMENT
                        | vk::ShaderStageFlags::COMPUTE,
                ),
        )
    },
    |device: Arc<Device>| {
        DescriptorSetLayout::new(
            device,
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        )
    },
];

pub const MAIN_PIPELINE: usize = 0;
pub const SKYBOX_PIPELINE: usize = 1;
pub const EGUI_PIPELINE: usize = 2;
pub const PIPELINES: [PipelineFunction; 3] = [
    make_main_pipeline,
    skybox::make_skybox_pipeline,
    egui_backend::make_egui_pipeline,
];
