use std::mem::offset_of;

use ash::vk;
use ultraviolet::{Vec2, Vec4};

use crate::{renderer::{device::{self, Device}, pipeline::{Pipeline, PipelineBuilder, VertexPushConstants}, shader_module::{spv, SpecializationInfo}, FOV}, vertex::Vertex};

fn make_main_pipeline(
    device: &Device,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    descriptor_set_layouts: &[vk::DescriptorSetLayout],
) -> Pipeline {
    let fov = FOV.to_radians();
    let ez = f32::tan(fov / 2.0).recip();
    let camera_parameters = Vec4::new(
        ez,
        (extent.width as f32) / (extent.height as f32),
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
                (&raw const camera_parameters).cast::<u8>(),
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

    let push_constant_ranges = [vk::PushConstantRange::default()
        .offset(0)
        .size(
            std::mem::size_of::<VertexPushConstants>()
                .try_into()
                .unwrap(),
        )
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
        .device(device.device.clone())
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

fn make_skybox_pipeline(
    device: &Device,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    descriptor_set_layouts: &[vk::DescriptorSetLayout],
) -> Pipeline {
    let fov = FOV.to_radians();
    let camera_parameters = Vec2::new(
        f32::tan(fov / 2.0),
        (extent.height as f32) / (extent.width as f32)
    );

    let fragment_specialization = SpecializationInfo::new(
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
            }
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
            device.device.clone(),
            "skybox.vert",
            vk::ShaderStageFlags::VERTEX,
            None
        ),
        spv!(
            device.device.clone(),
            "skybox.frag",
            vk::ShaderStageFlags::FRAGMENT,
            Some(fragment_specialization)
        ),
    ];

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

    let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
        .sample_shading_enable(false)
        .rasterization_samples(device.msaa_samples)
        .min_sample_shading(1.0);

    PipelineBuilder::new()
        .device(device.device.clone())
        .shader_stages(&shader_stages)
        .multisampling(&multisampling)
        .descriptor_set_layouts(&descriptor_set_layouts[0..1])
        .viewports(&viewport)
        .scissors(&scissor)
        .render_pass(render_pass)
        .build()
}

pub const PIPELINES: [for<'a, 'b> fn(
    &'a device::Device,
    vk::Extent2D,
    ash::vk::RenderPass,
    &'b [ash::vk::DescriptorSetLayout],
) -> Pipeline; 2] = [make_main_pipeline, make_skybox_pipeline];
