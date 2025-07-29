use std::{mem::offset_of, sync::Arc};

use ash::{
    Instance,
    vk::{self, SamplerAddressMode},
};
use egui::{Vec2, ahash::HashMap};

use crate::{
    clock::Clock,
    renderer::{
        Renderer,
        command_buffer::ActiveMultipleSubmitCommandBuffer,
        device::Device,
        image::Image,
        pipeline::{Pipeline, PipelineBuilder},
        sampler::Sampler,
        shader_module::{SpecializationInfo, spv},
    },
};

struct Texture {
    sampler: Sampler,
    image: Arc<Image>,
}

/// A Vulkan painter using ash + my renderer
pub struct EguiBackend {
    pub ctx: egui::Context,
    pub input: egui::RawInput,
    pub window_size: egui::Vec2,

    textures: HashMap<egui::TextureId, Texture>,
}

impl EguiBackend {
    pub fn new(gfx: &Renderer) -> Self {
        let mut input = egui::RawInput::default();

        let window_size = egui::Vec2::new(gfx.window_size.0 as f32, gfx.window_size.1 as f32);
        input.screen_rect = Some(egui::Rect::from_min_size(Default::default(), window_size));

        input.max_texture_side = unsafe {
            Some(
                gfx.instance
                    .get_physical_device_properties(gfx.device.physical_device)
                    .limits
                    .max_image_dimension2_d
                    .try_into()
                    .unwrap(),
            )
        };

        Self {
            ctx: egui::Context::default(),
            input,
            window_size,
            textures: HashMap::default(),
        }
    }

    pub fn draw(
        &mut self,
        instance: &Instance,
        device: &Arc<ash::Device>,
        physical_device: vk::PhysicalDevice,
        mut command_buffer: ActiveMultipleSubmitCommandBuffer,
    ) -> ActiveMultipleSubmitCommandBuffer {
        let full_output = self.ctx.run(self.input.clone(), |ctx| {
            egui::CentralPanel::default().show(&ctx, |ui| {
                ui.label("Hello world!");
                if ui.button("Click me").clicked() {
                    // take some action here
                }
            });
        });

        self.handle_platform_output(full_output.platform_output);

        let clipped_primitives = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        /*
        for primitive in clipped_primitives {
            match primitive.primitive {
                egui::epaint::Primitive::Mesh(mesh) => todo!(),
                egui::epaint::Primitive::Callback(paint_callback) => todo!(),
            }
        }

        for (tex_id, image_delta) in full_output.textures_delta.set {
            let tex = self.textures.get_mut(&tex_id);

            match tex {
                Some(_tex) => todo!(),
                None => match image_delta.pos {
                    Some(_pos) => todo!(),
                    None => {
                        let texture_filter =
                            |texture_filter: egui::TextureFilter| match texture_filter {
                                egui::TextureFilter::Nearest => vk::Filter::NEAREST,
                                egui::TextureFilter::Linear => vk::Filter::LINEAR,
                            };

                        let width = image_delta.image.width();
                        let height = image_delta.image.height();

                        let mip_levels = width.max(height).ilog2() + 1;

                        let sampler = Sampler::new(
                            instance,
                            device.clone(),
                            physical_device,
                            match image_delta.options.wrap_mode {
                                egui::TextureWrapMode::ClampToEdge => {
                                    SamplerAddressMode::CLAMP_TO_EDGE
                                }
                                egui::TextureWrapMode::Repeat => SamplerAddressMode::REPEAT,
                                egui::TextureWrapMode::MirroredRepeat => {
                                    SamplerAddressMode::MIRRORED_REPEAT
                                }
                            },
                            texture_filter(image_delta.options.magnification),
                            texture_filter(image_delta.options.minification),
                            false,
                            image_delta.options.mipmap_mode.map(|filter| {
                                (
                                    match filter {
                                        egui::TextureFilter::Nearest => {
                                            vk::SamplerMipmapMode::NEAREST
                                        }
                                        egui::TextureFilter::Linear => {
                                            vk::SamplerMipmapMode::LINEAR
                                        }
                                    },
                                    mip_levels,
                                )
                            }),
                        );

                        let image = match image_delta.image {
                            egui::ImageData::Color(color_image) => {
                                image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
                                    width as u32,
                                    height as u32,
                                    unsafe {
                                        std::slice::from_raw_parts(
                                            (&raw const color_image.pixels).cast::<u8>(),
                                            color_image.pixels.len() * size_of::<egui::Color32>(),
                                        )
                                        .to_vec()
                                    },
                                )
                            }
                        }
                        .unwrap();

                        let image = Image::from_image(
                            &instance,
                            physical_device,
                            device,
                            &mut command_buffer,
                            image.into(),
                        );

                        self.textures.insert(tex_id, Texture { sampler, image });
                    }
                },
            }
        }*/

        command_buffer
    }

    pub fn handle_platform_output(&mut self, platform_output: egui::PlatformOutput) {
        for event in platform_output.events {
            match event {
                egui::output::OutputEvent::Clicked(_widget_info) => (),
                egui::output::OutputEvent::DoubleClicked(_widget_info) => (),
                egui::output::OutputEvent::TripleClicked(_widget_info) => (),
                egui::output::OutputEvent::FocusGained(_widget_info) => (),
                egui::output::OutputEvent::TextSelectionChanged(_widget_info) => (),
                egui::output::OutputEvent::ValueChanged(_widget_info) => (),
            }
        }
    }

    pub fn update_input(
        &mut self,
        clock: &Clock,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) {
        self.input.screen_rect = Some(egui::Rect::from_min_size(
            Default::default(),
            self.window_size,
        ));

        self.input.time = Some(clock.time);
        self.input.modifiers = modifiers;
        self.input.events = events;
    }
}

pub fn make_egui_pipeline(
    device: &Device,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    descriptor_set_layouts: &[vk::DescriptorSetLayout],
) -> Pipeline {
    #[repr(C)]
    struct VertexParameters {
        width: f32,
        height: f32,
    }

    let extent_f32 = VertexParameters {
        width: extent.width as f32,
        height: extent.height as f32,
    };

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
                (&raw const extent_f32).cast::<u8>(),
                std::mem::size_of::<Vec2>(),
            )
        },
    );

    let shader_stages = [
        spv!(
            device.device.clone(),
            "egui.vert",
            vk::ShaderStageFlags::VERTEX,
            Some(vertex_specialization)
        ),
        spv!(
            device.device.clone(),
            "egui.frag",
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

    let vertex_binding_descriptions = [vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(size_of::<egui::epaint::Vertex>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)];

    let vertex_attribute_descriptions = [
        vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: offset_of!(egui::epaint::Vertex, pos) as u32,
        },
        vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: offset_of!(egui::epaint::Vertex, uv) as u32,
        },
        vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UINT,
            offset: offset_of!(egui::epaint::Vertex, color) as u32,
        },
    ];

    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&vertex_binding_descriptions)
        .vertex_attribute_descriptions(&vertex_attribute_descriptions);

    PipelineBuilder::new()
        .device(device.device.clone())
        .render_pass(render_pass)
        .descriptor_set_layouts(descriptor_set_layouts)
        .shader_stages(&shader_stages)
        .viewports(&viewport)
        .scissors(&scissor)
        .multisampling(&multisampling)
        .vertex_input_info(&vertex_input_info)
        .build()
}
