use std::{mem::offset_of, sync::Arc};

use ash::vk;
use easy_cast::{Cast, CastFloat};
use egui::{ClippedPrimitive, Vec2, ahash::HashMap};

use crate::{
    clock::Clock,
    renderer::{
        Renderer,
        buffer::Buffer,
        command_buffer::ActiveCommandBuffer,
        descriptors::DescriptorSet,
        device::Device,
        image::Image,
        pipeline::{Pipeline, PipelineBuilder},
        sampler::Sampler,
        shader_module::{SpecializationInfo, spv},
    },
    shader_pipelines::MATERIAL_LAYOUT,
};

/// A Vulkan painter using ash + my renderer
pub struct EguiBackend {
    pub ctx: egui::Context,
    pub input: egui::RawInput,
    pub window_size: egui::Vec2,

    full_output: Option<egui::FullOutput>,

    clipped_primitives: Vec<ClippedPrimitive>,
    index_offset: usize,
    vertex_index_buffer: Option<Arc<Buffer<u8>>>,

    textures: HashMap<egui::TextureId, Texture>,
}

#[allow(dead_code)]
struct Texture {
    image: Arc<Image>,
    sampler: Arc<Sampler>,
    descriptor_set: DescriptorSet,
}

fn texture_filter(texture_filter: egui::TextureFilter) -> vk::Filter {
    match texture_filter {
        egui::TextureFilter::Nearest => vk::Filter::NEAREST,
        egui::TextureFilter::Linear => vk::Filter::LINEAR,
    }
}

fn mipmap_filter(mipmap_filter: egui::TextureFilter) -> vk::SamplerMipmapMode {
    match mipmap_filter {
        egui::TextureFilter::Nearest => vk::SamplerMipmapMode::NEAREST,
        egui::TextureFilter::Linear => vk::SamplerMipmapMode::LINEAR,
    }
}

fn wrap_mode(wrap_mode: egui::TextureWrapMode) -> vk::SamplerAddressMode {
    match wrap_mode {
        egui::TextureWrapMode::ClampToEdge => vk::SamplerAddressMode::CLAMP_TO_EDGE,
        egui::TextureWrapMode::Repeat => vk::SamplerAddressMode::REPEAT,
        egui::TextureWrapMode::MirroredRepeat => vk::SamplerAddressMode::MIRRORED_REPEAT,
    }
}

impl Texture {
    fn update<C: ActiveCommandBuffer>(
        &mut self,
        gfx: &Renderer,
        image_delta: &egui::epaint::ImageDelta,
        command_buffer: &mut C,
    ) {
        let region = vk::Rect2D {
            offset: image_delta
                .pos
                .map_or(vk::Offset2D::default(), |[x, y]| vk::Offset2D {
                    x: x.cast(),
                    y: y.cast(),
                }),
            extent: self.image.extent,
        };

        let data = match &image_delta.image {
            egui::ImageData::Color(color_image) => color_image
                .pixels
                .iter()
                .flat_map(egui::Color32::to_array)
                .collect::<Vec<_>>(),
        };

        let staging_buffer = Arc::new(Buffer::new(
            &gfx.device,
            &data,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        ));

        self.image.transition_layout(
            &gfx.device,
            command_buffer,
            None,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );

        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D {
                x: region.offset.x,
                y: region.offset.y,
                z: 0,
            })
            .image_extent(vk::Extent3D {
                width: region.extent.width,
                height: region.extent.height,
                depth: 1,
            });

        unsafe {
            gfx.device.cmd_copy_buffer_to_image(
                **command_buffer,
                staging_buffer.buffer,
                self.image.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
        }

        let mipmapping = self.image.mip_levels > 1;

        if mipmapping {
            self.image.generate_mipmaps(&gfx.device, command_buffer);
        }

        command_buffer.add_dependency(staging_buffer);

        self.image.transition_layout(
            &gfx.device,
            command_buffer,
            None,
            if mipmapping {
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL
            } else {
                vk::ImageLayout::TRANSFER_DST_OPTIMAL
            },
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
    }

    fn new<C: ActiveCommandBuffer>(
        gfx: &Renderer,
        image_delta: &egui::epaint::ImageDelta,
        command_buffer: &mut C,
    ) -> Texture {
        let width = image_delta.image.width();
        let height = image_delta.image.height();

        let mip_levels = width.max(height).ilog2() + 1;

        let sampler = Arc::new(Sampler::new(
            gfx.device.clone(),
            wrap_mode(image_delta.options.wrap_mode),
            texture_filter(image_delta.options.magnification),
            texture_filter(image_delta.options.minification),
            false,
            image_delta
                .options
                .mipmap_mode
                .map(|filter| (mipmap_filter(filter), mip_levels)),
        ));

        let image = match &image_delta.image {
            egui::ImageData::Color(color_image) => {
                image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
                    width.cast(),
                    height.cast(),
                    color_image
                        .pixels
                        .iter()
                        .flat_map(egui::Color32::to_array)
                        .collect::<Vec<_>>(),
                )
            }
        }
        .unwrap();

        let image = Image::from_image(&gfx.device, command_buffer, image.into(), false);

        let mut descriptor_set = gfx
            .descriptor_pool
            .create_descriptor_set(&gfx.descriptor_set_layouts[MATERIAL_LAYOUT]);

        descriptor_set.bind_texture(&gfx.device.device, 0, image.clone(), sampler.clone());

        Texture {
            image,
            sampler,
            descriptor_set,
        }
    }
}

impl EguiBackend {
    pub fn gui_scale(&mut self, gui_scale: f32) {
        self.ctx.set_zoom_factor(gui_scale);
    }

    pub fn new(gfx: &Renderer) -> Self {
        let mut input = egui::RawInput::default();

        let window_size = egui::Vec2::new(gfx.window_size.0.cast(), gfx.window_size.1.cast());
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::default(),
            window_size,
        ));

        input.max_texture_side = unsafe {
            Some(
                gfx.device
                    .instance
                    .get_physical_device_properties(gfx.device.physical_device)
                    .limits
                    .max_image_dimension2_d
                    .try_into()
                    .unwrap(),
            )
        };

        let ctx = egui::Context::default();

        ctx.set_visuals(egui::Visuals::dark());

        Self {
            ctx,
            input,
            window_size,
            full_output: None,
            textures: HashMap::default(),
            index_offset: 0,
            clipped_primitives: Vec::new(),
            vertex_index_buffer: None,
        }
    }

    pub fn run(&mut self) {
        #[derive(PartialEq)]
        enum Enum {
            First,
            Second,
            Third,
        }

        let mut my_string = String::new();
        let mut my_f32 = 0.0f32;
        let mut my_boolean = false;
        let mut my_enum = Enum::First;

        let full_output = self.ctx.run(self.input.clone(), |ctx| {
            egui::SidePanel::left("my_left_panel")
                .frame(egui::Frame {
                    inner_margin: egui::Margin::symmetric(4, 4),
                    fill: egui::Color32::from_black_alpha(200),
                    stroke: egui::Stroke::NONE,
                    corner_radius: egui::CornerRadius::ZERO,
                    outer_margin: egui::Margin::ZERO,
                    shadow: egui::Shadow::NONE,
                })
                .show(ctx, |ui| {
                    ui.label("This is a label");
                    ui.hyperlink("https://github.com/emilk/egui");
                    ui.text_edit_singleline(&mut my_string);
                    if ui.button("Click me").clicked() {
                        println!("Clicked!!");
                    }
                    ui.add(egui::Slider::new(&mut my_f32, 0.0..=100.0));
                    ui.add(egui::DragValue::new(&mut my_f32));

                    ui.checkbox(&mut my_boolean, "Checkbox");

                    ui.horizontal(|ui| {
                        ui.radio_value(&mut my_enum, Enum::First, "First");
                        ui.radio_value(&mut my_enum, Enum::Second, "Second");
                        ui.radio_value(&mut my_enum, Enum::Third, "Third");
                    });

                    ui.separator();

                    ui.collapsing("Click to see what is hidden!", |ui| {
                        ui.label("Not much, as it turns out");
                    });
                });
        });

        self.input.events.clear();

        self.handle_platform_output(&full_output.platform_output);

        self.full_output = Some(full_output);
    }

    pub fn free_textures(&mut self) {
        if let Some(full_output) = &self.full_output {
            log::debug!("freeing {} textures", full_output.textures_delta.free.len());
            for tex in &full_output.textures_delta.free {
                self.textures.remove(tex);
            }
        }
    }

    pub fn update_textures(&mut self, gfx: &Renderer) {
        let full_output = self
            .full_output
            .as_ref()
            .expect("egui_backend: run must be called before update_textures");

        if full_output.textures_delta.is_empty() {
            return;
        }

        log::debug!("adding new texture");
        gfx.command_pool
            .one_time_submit(gfx.device.graphics_queue, |command_buffer| {
                for (tex_id, image_delta) in &full_output.textures_delta.set {
                    self.textures
                        .entry(*tex_id)
                        .and_modify(|tex| tex.update(gfx, image_delta, command_buffer))
                        .or_insert(if let Some(_pos) = image_delta.pos {
                            todo!()
                        } else {
                            Texture::new(gfx, image_delta, command_buffer)
                        });
                }
            });
    }

    pub fn upload_clipped_primitives(&mut self, gfx: &Renderer) {
        let full_output = self
            .full_output
            .as_ref()
            .expect("egui_backend: run must be called before upload_clipped_primitives");

        self.clipped_primitives = self
            .ctx
            .tessellate(full_output.shapes.clone(), full_output.pixels_per_point);

        let mut index_buffers = Vec::new();
        let mut vertex_buffers = Vec::new();

        for primitive in &self.clipped_primitives {
            match &primitive.primitive {
                egui::epaint::Primitive::Mesh(mesh) => {
                    index_buffers.extend(mesh.indices.clone());
                    vertex_buffers.extend(mesh.vertices.clone());
                }
                egui::epaint::Primitive::Callback(_paint_callback) => {
                    todo!("callback primitives not supported")
                }
            }
        }

        let index_byte_length = index_buffers.len() * size_of::<u32>();
        let vertex_byte_length = vertex_buffers.len() * size_of::<egui::epaint::Vertex>();
        self.index_offset = vertex_byte_length;

        self.vertex_index_buffer = Some(gfx.command_pool.one_time_submit(
            gfx.device.graphics_queue,
            |cmd_buf| {
                Buffer::new_staged_with(
                    &gfx.device,
                    cmd_buf,
                    vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
                    |mapped_memory: &mut [u8]| {
                        mapped_memory[0..vertex_byte_length].copy_from_slice(unsafe {
                            std::slice::from_raw_parts(
                                vertex_buffers.as_ptr().cast::<u8>(),
                                vertex_byte_length,
                            )
                        });

                        mapped_memory[vertex_byte_length..].copy_from_slice(unsafe {
                            std::slice::from_raw_parts(
                                index_buffers.as_ptr().cast::<u8>(),
                                index_byte_length,
                            )
                        });
                    },
                    vertex_byte_length + index_byte_length,
                )
            },
        ));
    }

    pub fn draw(
        &mut self,
        device: &Device,
        extent: vk::Extent2D,
        cmd_buf: vk::CommandBuffer,
        pipeline: &Pipeline,
    ) {
        let full_output = self
            .full_output
            .take()
            .expect("egui_backend: run must be called before draw");

        let pixels_per_point = full_output.pixels_per_point;

        let mut current_texture_id = None;

        let mut vertex_offset = 0;
        let mut index_offest = 0;

        unsafe {
            device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
        }

        let vertex_index_buffer = self.vertex_index_buffer.as_ref().unwrap().buffer;

        unsafe {
            device.cmd_bind_index_buffer(
                cmd_buf,
                vertex_index_buffer,
                self.index_offset.cast(),
                vk::IndexType::UINT32,
            );

            device.cmd_bind_vertex_buffers(cmd_buf, 0, &[vertex_index_buffer], &[0]);
        }

        unsafe {
            device.cmd_push_constants(
                cmd_buf,
                pipeline.pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                &self.ctx.pixels_per_point().to_ne_bytes(),
            );
        }

        let mut draw_primitive =
            |mesh: &egui::epaint::Mesh, primitive: &egui::epaint::ClippedPrimitive| {
                let clip_rect = primitive.clip_rect;

                let clip_x: i32 = (clip_rect.min.x * pixels_per_point).cast_nearest();
                let clip_y: i32 = (clip_rect.min.y * pixels_per_point).cast_nearest();
                let clip_w: i32 = (clip_rect.max.x * pixels_per_point).cast_nearest();
                let clip_h: i32 = (clip_rect.max.y * pixels_per_point).cast_nearest();

                unsafe {
                    device.cmd_set_scissor(
                        cmd_buf,
                        0,
                        &[vk::Rect2D {
                            offset: vk::Offset2D {
                                x: clip_x.clamp(0, extent.width.cast()),
                                y: clip_y.clamp(0, extent.height.cast()),
                            },
                            extent: vk::Extent2D {
                                width: (clip_w.clamp(clip_x, extent.width.cast()) - clip_x).cast(),
                                height: (clip_h.clamp(clip_y, extent.height.cast()) - clip_y)
                                    .cast(),
                            },
                        }],
                    );

                    if let Some(current_texture_id) = current_texture_id
                        && current_texture_id == mesh.texture_id
                    {
                    } else {
                        device.cmd_bind_descriptor_sets(
                            cmd_buf,
                            vk::PipelineBindPoint::GRAPHICS,
                            pipeline.pipeline_layout,
                            1,
                            &[*self.textures[&mesh.texture_id].descriptor_set],
                            &[],
                        );
                    }

                    device.cmd_draw_indexed(
                        cmd_buf,
                        mesh.indices.len().cast(),
                        1,
                        index_offest.cast(),
                        vertex_offset.cast(),
                        0,
                    );

                    vertex_offset += mesh.vertices.len();
                    index_offest += mesh.indices.len();
                };

                current_texture_id = Some(mesh.texture_id);
            };

        for primitive in &self.clipped_primitives {
            match &primitive.primitive {
                egui::epaint::Primitive::Mesh(mesh) => draw_primitive(mesh, primitive),
                egui::epaint::Primitive::Callback(_paint_callback) => {
                    todo!("callback primitives not supported")
                }
            }
        }
    }

    #[allow(clippy::unused_self)]
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn handle_platform_output(&mut self, platform_output: &egui::PlatformOutput) {
        for event in &platform_output.events {
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
            egui::Pos2::default(),
            self.window_size,
        ));

        self.input.time = Some(clock.time);
        self.input.modifiers = modifiers;
        self.input.events.extend(events);
    }
}

pub fn make_egui_pipeline(
    device: &Arc<Device>,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    descriptor_set_layouts: &[vk::DescriptorSetLayout],
) -> Pipeline {
    let extent_f32 = ultraviolet::Vec2::new(extent.width.cast(), extent.height.cast());

    let info = [
        vk::SpecializationMapEntry {
            constant_id: 0,
            offset: offset_of!(Vec2, x).cast(),
            size: std::mem::size_of::<f32>(),
        },
        vk::SpecializationMapEntry {
            constant_id: 1,
            offset: offset_of!(Vec2, y).cast(),
            size: std::mem::size_of::<f32>(),
        },
    ];

    let vertex_specialization = SpecializationInfo::new(&info, unsafe {
        std::slice::from_raw_parts(
            (&raw const extent_f32).cast::<u8>(),
            std::mem::size_of::<Vec2>(),
        )
    });

    let shader_stages = [
        spv!(
            device.clone(),
            "egui.vert",
            vk::ShaderStageFlags::VERTEX,
            Some(vertex_specialization)
        ),
        spv!(
            device.clone(),
            "egui.frag",
            vk::ShaderStageFlags::FRAGMENT,
            None
        ),
    ];

    let viewport = [vk::Viewport::default()
        .x(0.0)
        .y(0.0)
        .width(extent.width.cast())
        .height(extent.height.cast())
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
        .stride(size_of::<egui::epaint::Vertex>().cast())
        .input_rate(vk::VertexInputRate::VERTEX)];

    let vertex_attribute_descriptions = [
        vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: offset_of!(egui::epaint::Vertex, pos).cast(),
        },
        vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: offset_of!(egui::epaint::Vertex, uv).cast(),
        },
        vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R8G8B8A8_UNORM,
            offset: offset_of!(egui::epaint::Vertex, color).cast(),
        },
    ];

    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&vertex_binding_descriptions)
        .vertex_attribute_descriptions(&vertex_attribute_descriptions);

    let dynamic_states = [vk::DynamicState::SCISSOR];

    let color_blend_attachment = [vk::PipelineColorBlendAttachmentState {
        blend_enable: vk::TRUE,
        src_color_blend_factor: vk::BlendFactor::ONE,
        dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        color_blend_op: vk::BlendOp::ADD,
        src_alpha_blend_factor: vk::BlendFactor::ONE_MINUS_DST_ALPHA,
        dst_alpha_blend_factor: vk::BlendFactor::ONE,
        alpha_blend_op: vk::BlendOp::ADD,
        color_write_mask: vk::ColorComponentFlags::RGBA,
    }];

    let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(&color_blend_attachment);

    let push_constant_ranges = [vk::PushConstantRange::default()
        .offset(0)
        .size(std::mem::size_of::<f32>().cast())
        .stage_flags(vk::ShaderStageFlags::VERTEX)];

    PipelineBuilder::new()
        .device(device.clone())
        .render_pass(render_pass)
        .descriptor_set_layouts(descriptor_set_layouts)
        .shader_stages(&shader_stages)
        .viewports(&viewport)
        .scissors(&scissor)
        .multisampling(&multisampling)
        .vertex_input_info(&vertex_input_info)
        .dynamic_states(&dynamic_states)
        .color_blending(&color_blending)
        .push_constant_ranges(&push_constant_ranges)
        .build()
}
