use std::sync::Arc;

use ash::vk;

const SKYBOX_RESOLUTION: vk::Extent2D = vk::Extent2D {
    width: WIDTH / 2,
    height: HEIGHT / 2,
};

use crate::{
    renderer::{
        HEIGHT, Renderer, WIDTH,
        command_buffer::ActiveMultipleSubmitCommandBuffer,
        descriptors::{DescriptorSet, DescriptorSetLayout},
        device::Device,
        image::Image,
        material::{Material, MaterialProperties},
        pipeline::{ComputePipelineBuilder, Pipeline, PipelineBuilder},
        sampler::Sampler,
        shader_module::spv,
    },
    shader_pipelines::{MATERIAL_LAYOUT, UNIFORM_BUFFER_LAYOUT},
};

pub struct Skybox {
    image: Arc<Image>,
    texture: Material,
    compute_pipeline: Pipeline,
    descriptor: DescriptorSet,
}

impl Skybox {
    pub fn render(
        &self,
        device: &Device,
        command_buffer: ActiveMultipleSubmitCommandBuffer,
        ubo: &DescriptorSet,
    ) -> ActiveMultipleSubmitCommandBuffer {
        let mut command_buffer = command_buffer;
        let cmd_buf = *command_buffer;

        unsafe {
            self.image.transition_layout(
                device,
                &mut command_buffer,
                None,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::GENERAL,
            );

            let skybox_pipeline_descriptor_sets = [ubo.descriptor_set, *self.descriptor];
            device.cmd_bind_pipeline(
                cmd_buf,
                vk::PipelineBindPoint::COMPUTE,
                *self.compute_pipeline,
            );
            device.cmd_bind_descriptor_sets(
                cmd_buf,
                vk::PipelineBindPoint::COMPUTE,
                self.compute_pipeline.pipeline_layout,
                0,
                &skybox_pipeline_descriptor_sets,
                &[],
            );

            device.cmd_dispatch(
                cmd_buf,
                self.image.extent.width.div_ceil(8),
                self.image.extent.height.div_ceil(8),
                1,
            );

            self.image.transition_layout(
                &device.device,
                &mut command_buffer,
                None,
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        }

        command_buffer
    }

    pub fn blit(
        &self,
        device: &Device,
        command_buffer: ActiveMultipleSubmitCommandBuffer,
        graphics_pipeline: &Pipeline,
        ubo: &DescriptorSet,
    ) -> ActiveMultipleSubmitCommandBuffer {
        unsafe {
            let skybox_pipeline_descriptor_sets =
                [ubo.descriptor_set, *self.texture.descriptor_set];
            device.cmd_bind_pipeline(
                *command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                **graphics_pipeline,
            );
            device.cmd_bind_descriptor_sets(
                *command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                graphics_pipeline.pipeline_layout,
                0,
                &skybox_pipeline_descriptor_sets,
                &[],
            );
            device.cmd_draw(*command_buffer, 3, 1, 0, 0);
        }

        command_buffer
    }

    pub fn new(gfx: &Renderer) -> Self {
        let image = gfx
            .command_pool
            .one_time_submit(gfx.device.graphics_queue, |cmd_buf| {
                Arc::new(Image::new_with_layout(
                    &gfx.instance,
                    gfx.device.physical_device,
                    &gfx.device.device,
                    SKYBOX_RESOLUTION,
                    vk::SampleCountFlags::TYPE_1,
                    vk::Format::R8G8B8A8_UNORM,
                    vk::ImageTiling::OPTIMAL,
                    vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                    vk::ImageAspectFlags::COLOR,
                    cmd_buf,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ))
            });

        let texture_layout = {
            let texture_bindings = vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .stage_flags(vk::ShaderStageFlags::COMPUTE);

            DescriptorSetLayout::new(gfx.device.device.clone(), texture_bindings)
        };

        let descriptor_set_layouts = [
            gfx.descriptor_set_layouts[UNIFORM_BUFFER_LAYOUT].layout,
            texture_layout.layout,
        ];

        let shader = spv!(
            gfx.device.device.clone(),
            "skybox.comp",
            vk::ShaderStageFlags::COMPUTE,
            None
        );

        let mut descriptor = gfx.descriptor_pool.create_descriptor_set(&texture_layout);

        descriptor.bind_image(&gfx.device.device, 0, image.clone());

        let skybox_sampler = Arc::new(Sampler::new(
            &gfx.instance,
            gfx.device.device.clone(),
            gfx.device.physical_device,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            false,
            0,
        ));

        let texture = Material::new(
            &gfx.device,
            image.clone(),
            skybox_sampler,
            MaterialProperties::default(),
            &gfx.descriptor_pool,
            &gfx.descriptor_set_layouts[MATERIAL_LAYOUT],
        );

        let compute_pipeline = ComputePipelineBuilder::new()
            .device(gfx.device.device.clone())
            .layouts(&descriptor_set_layouts)
            .shader(&shader)
            .build();

        Self {
            image,
            texture,
            compute_pipeline,
            descriptor,
        }
    }
}

pub fn make_skybox_pipeline(
    device: &Device,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    descriptor_set_layouts: &[vk::DescriptorSetLayout],
) -> Pipeline {
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
            None
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
        .descriptor_set_layouts(descriptor_set_layouts)
        .viewports(&viewport)
        .scissors(&scissor)
        .render_pass(render_pass)
        .build()
}
