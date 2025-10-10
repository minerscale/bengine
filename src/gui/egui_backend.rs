use std::{mem::offset_of, sync::Arc};

use ash::vk;
use easy_cast::{Cast, CastFloat};
use egui::{ClippedPrimitive, Vec2, ahash::HashMap};

use crate::{
    clock::Clock,
    event_loop::SharedState,
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

pub type GuiFn = dyn FnMut(&egui::Context, &mut SharedState) + Send + Sync;

pub struct EguiBackend {
    pub ctx: egui::Context,
    pub input: egui::RawInput,
    pub window_size: egui::Vec2,

    full_output: Option<egui::FullOutput>,

    clipped_primitives: Vec<ClippedPrimitive>,
    index_offset: usize,
    vertex_index_buffer: Option<Arc<Buffer<u8>>>,

    textures: HashMap<egui::TextureId, Texture>,

    last_gui_scale: f32,

    gui_fn: Box<GuiFn>,
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
            extent: vk::Extent2D {
                width: image_delta.image.width().cast(),
                height: image_delta.image.height().cast(),
            },
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

        let mip_levels = if let Some(_mode) = image_delta.options.mipmap_mode {
            width.max(height).ilog2() + 1
        } else {
            1
        };

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

        let image = Image::from_image(
            &gfx.device,
            command_buffer,
            image.into(),
            true,
            image_delta.options.mipmap_mode.is_some(),
        );

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
    fn theme() -> egui::Visuals {
        egui::Visuals {
            dark_mode: true,
            text_alpha_from_coverage: egui::epaint::AlphaFromCoverage::DARK_MODE_DEFAULT,
            override_text_color: None,
            weak_text_alpha: 0.6,
            weak_text_color: None,
            widgets: egui::style::Widgets {
                noninteractive: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_gray(27),
                    bg_fill: egui::Color32::from_gray(27),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_gray(60)), // separators, indentation lines
                    fg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(178, 191, 219)), // normal text color
                    corner_radius: egui::CornerRadius::same(2),
                    expansion: 0.0,
                },
                inactive: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_rgb(60, 51, 35), // button background
                    bg_fill: egui::Color32::from_rgb(60, 51, 35),      // checkbox background
                    bg_stroke: Default::default(),
                    fg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(178, 191, 219)), // button text
                    corner_radius: egui::CornerRadius::same(2),
                    expansion: 0.0,
                },
                hovered: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_gray(70),
                    bg_fill: egui::Color32::from_gray(70),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_gray(150)), // e.g. hover over window edge or button
                    fg_stroke: egui::Stroke::new(1.5, egui::Color32::from_gray(240)),
                    corner_radius: egui::CornerRadius::same(3),
                    expansion: 1.0,
                },
                active: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_gray(55),
                    bg_fill: egui::Color32::from_gray(55),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::WHITE),
                    fg_stroke: egui::Stroke::new(2.0, egui::Color32::WHITE),
                    corner_radius: egui::CornerRadius::same(2),
                    expansion: 1.0,
                },
                open: egui::style::WidgetVisuals {
                    weak_bg_fill: egui::Color32::from_gray(45),
                    bg_fill: egui::Color32::from_gray(27),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
                    fg_stroke: egui::Stroke::new(1.0, egui::Color32::from_gray(210)),
                    corner_radius: egui::CornerRadius::same(2),
                    expansion: 0.0,
                },
            },
            selection: egui::style::Selection::default(),
            hyperlink_color: egui::Color32::from_rgb(90, 170, 255),
            faint_bg_color: egui::Color32::from_additive_luminance(5), // visible, but barely so
            extreme_bg_color: egui::Color32::from_gray(10),            // e.g. TextEdit background
            text_edit_bg_color: None, // use `extreme_bg_color` by default
            code_bg_color: egui::Color32::from_gray(64),
            warn_fg_color: egui::Color32::from_rgb(255, 143, 0), // orange
            error_fg_color: egui::Color32::from_rgb(255, 0, 0),  // red

            window_corner_radius: egui::CornerRadius::same(6),
            window_shadow: egui::Shadow {
                offset: [10, 20],
                blur: 15,
                spread: 0,
                color: egui::Color32::from_black_alpha(96),
            },
            window_fill: egui::Color32::from_gray(27),
            window_stroke: egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
            window_highlight_topmost: true,

            menu_corner_radius: egui::CornerRadius::same(6),

            panel_fill: egui::Color32::from_gray(27),

            popup_shadow: egui::Shadow {
                offset: [6, 10],
                blur: 8,
                spread: 0,
                color: egui::Color32::from_black_alpha(96),
            },

            resize_corner_size: 12.0,

            text_cursor: Default::default(),

            clip_rect_margin: 3.0, // should be at least half the size of the widest frame stroke + max WidgetVisuals::expansion
            button_frame: true,
            collapsing_header_frame: false,
            indent_has_left_vline: true,

            striped: false,

            slider_trailing_fill: false,
            handle_shape: egui::style::HandleShape::Rect { aspect_ratio: 0.75 },

            interact_cursor: None,

            image_loading_spinners: true,

            numeric_color_space: egui::style::NumericColorSpace::GammaByte,
            disabled_alpha: 0.5,
        }
    }

    fn gui_scale(&mut self, gui_scale: f32) {
        if gui_scale != self.last_gui_scale {
            self.ctx.set_zoom_factor(gui_scale);
            self.last_gui_scale = gui_scale;
        }
    }

    pub fn new(gfx: &Renderer, gui_fn: Box<GuiFn>) -> Self {
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

        let mut fonts = egui::FontDefinitions::default();

        // Install my own font (maybe supporting non-latin characters):
        fonts.font_data.insert(
            "libertinus".to_owned(),
            std::sync::Arc::new(
                // .ttf and .otf supported
                egui::FontData::from_static(include_bytes!(
                    "../../test-objects/LibertinusSerifDisplay-Regular.otf"
                )),
            ),
        );

        // Put my font first (highest priority):
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "libertinus".to_owned());

        // Put my font as last fallback for monospace:
        fonts
            .families
            .get_mut(&egui::FontFamily::Monospace)
            .unwrap()
            .push("libertinus".to_owned());

        ctx.set_fonts(fonts);

        ctx.set_visuals(Self::theme());

        egui_extras::install_image_loaders(&ctx);

        Self {
            ctx,
            input,
            window_size,
            full_output: None,
            textures: HashMap::default(),
            index_offset: 0,
            clipped_primitives: Vec::new(),
            vertex_index_buffer: None,
            last_gui_scale: 1.0,
            gui_fn,
        }
    }

    pub fn update(&mut self, gfx: &Renderer, shared_state: &mut SharedState) {
        self.gui_scale(shared_state.gui_scale);
        self.free_textures();
        self.run(shared_state);
        self.update_textures(gfx);
        self.upload_clipped_primitives(gfx);
    }

    fn run(&mut self, shared_state: &mut SharedState) {
        let full_output = self
            .ctx
            .run(self.input.clone(), |ctx| (self.gui_fn)(ctx, shared_state));

        self.input.events.clear();

        self.handle_platform_output(&full_output.platform_output);

        self.full_output = Some(full_output);
    }

    fn free_textures(&mut self) {
        if let Some(full_output) = &self.full_output {
            log::debug!("freeing {} textures", full_output.textures_delta.free.len());
            for tex in &full_output.textures_delta.free {
                self.textures.remove(tex);
            }
        }
    }

    fn update_textures(&mut self, gfx: &Renderer) {
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
                        .and_modify(|tex| {
                            if image_delta.is_whole() {
                                command_buffer.add_dependency(tex.image.clone());
                                command_buffer.add_dependency(tex.sampler.clone());
                                *tex = Texture::new(gfx, image_delta, command_buffer)
                            } else {
                                tex.update(gfx, image_delta, command_buffer)
                            }
                        })
                        .or_insert_with(|| {
                            if let Some(_pos) = image_delta.pos {
                                unimplemented!()
                            } else {
                                Texture::new(gfx, image_delta, command_buffer)
                            }
                        });
                }
            });
    }

    fn upload_clipped_primitives(&mut self, gfx: &Renderer) {
        let full_output = self
            .full_output
            .as_ref()
            .expect("egui_backend: run must be called before upload_clipped_primitives");

        self.clipped_primitives = self
            .ctx
            .tessellate(full_output.shapes.clone(), full_output.pixels_per_point);

        if self.clipped_primitives.is_empty() {
            return;
        }

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

        let vertex_index_buffer = match self.vertex_index_buffer.as_ref() {
            Some(buf) => buf.buffer,
            None => return,
        };

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
    fn handle_platform_output(&mut self, platform_output: &egui::PlatformOutput) {
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
            self.window_size / self.ctx.pixels_per_point(),
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
