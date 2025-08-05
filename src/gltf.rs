use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Cursor},
    path::Path,
    sync::Arc,
};

use ash::vk;
use easy_cast::Cast;
use gltf::Gltf;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use ultraviolet::Vec3;

use crate::{
    mesh::{Mesh, Primitive},
    node::Node,
    renderer::{
        Renderer,
        buffer::{Buffer, BufferMemory},
        command_buffer::OneTimeSubmitCommandBuffer,
        image::Image,
        material::{Material, MaterialProperties},
        sampler::Sampler,
    },
    shader_pipelines::MATERIAL_LAYOUT,
    vertex::Vertex,
};

fn get_uri(view: &gltf::buffer::View) -> String {
    view.index().to_string() + &view.offset().to_string()
}

fn extend_align(buffer: &mut Vec<u8>, align: usize) {
    if align > 0 {
        let remainder = buffer.len() % align;

        buffer.extend(std::iter::repeat_n(0, align - remainder));
    }
}

struct MeshInfo {
    vertex_buffer: Buffer<Vertex>,
    index_buffer: Buffer<u32>,
    material: Arc<Material>,
    vertex_offset: vk::DeviceSize,
    index_offset: vk::DeviceSize,
}

pub fn load_gltf(
    gfx: &Renderer,
    cmd_buf: &mut OneTimeSubmitCommandBuffer,
    file: Result<&str, &[u8]>,
    scale: f32,
) -> Node {
    let current_dir = Path::new(".");

    let (root, gltf) = match file {
        Ok(filename) => {
            let root = Path::new(filename)
                .parent()
                .unwrap_or_else(|| Path::new("."));

            let gltf = Gltf::from_reader(BufReader::new(
                File::open("test-objects/Sponza.gltf").unwrap(),
            ))
            .unwrap();

            (root, gltf)
        }
        Err(file) => (current_dir, Gltf::from_slice(file).unwrap()),
    };

    let buffers = gltf::import_buffers(&gltf.document, Some(root), gltf.blob).unwrap();
    let document = gltf.document;

    let images: HashMap<String, Arc<Image>> = document
        .images()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|image| {
            let (uri, image) = match image.source() {
                gltf::image::Source::View { view, mime_type: _ } => {
                    let start = view.offset();
                    let end = view.offset() + view.length();
                    let buffer = &buffers[view.buffer().index()][start..end];

                    (
                        get_uri(&view),
                        image::ImageReader::new(Cursor::new(buffer))
                            .with_guessed_format()
                            .unwrap()
                            .decode()
                            .unwrap(),
                    )
                }
                gltf::image::Source::Uri { uri, mime_type: _ } => (
                    uri.to_owned(),
                    image::ImageReader::new(BufReader::new(
                        File::open(root.join(Path::new(uri))).unwrap(),
                    ))
                    .with_guessed_format()
                    .unwrap()
                    .decode()
                    .unwrap(),
                ),
            };

            (uri, image)
        })
        .collect::<Box<_>>()
        .into_iter()
        .map(|(uri, image)| (uri, Image::from_image(&gfx.device, cmd_buf, image, true)))
        .collect();

    let materials = document
        .materials()
        .map(|material| {
            let image = match material
                .pbr_metallic_roughness()
                .base_color_texture()
                .unwrap()
                .texture()
                .source()
                .source()
            {
                gltf::image::Source::View { view, mime_type: _ } => &images[&get_uri(&view)],
                gltf::image::Source::Uri { uri, mime_type: _ } => &images[uri],
            };

            let properties = MaterialProperties {
                alpha_cutoff: material.alpha_cutoff().unwrap_or(0.0),
            };

            Arc::new(Material::new(
                &gfx.device,
                image.clone(),
                Arc::new(Sampler::new(
                    gfx.device.clone(),
                    vk::SamplerAddressMode::REPEAT,
                    vk::Filter::LINEAR,
                    vk::Filter::LINEAR,
                    true,
                    Some((vk::SamplerMipmapMode::LINEAR, image.mip_levels)),
                )),
                properties,
                &gfx.descriptor_pool,
                &gfx.descriptor_set_layouts[MATERIAL_LAYOUT],
            ))
        })
        .collect::<Vec<_>>();

    let mut vertex_buffers: Vec<u8> = Vec::new();
    let mut index_buffers: Vec<u8> = Vec::new();

    let mut first_index_align: Option<vk::DeviceSize> = None;

    let mesh_info = document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .map(|primitive| {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let vertexes = reader
                .read_positions()
                .unwrap()
                .zip(reader.read_normals().unwrap())
                .zip(reader.read_tex_coords(0).unwrap().into_f32())
                .map(|((position, normal), tex_coord)| {
                    Vertex::new(
                        Vec3::from(position) * scale,
                        normal.into(),
                        tex_coord.into(),
                    )
                });

            let vertex_size = vertexes.len();

            let indices = reader.read_indices().unwrap().into_u32();
            let index_size = indices.len();

            let (vertex_buffer, index_buffer) = unsafe {
                (
                    Buffer::new_uninit(
                        gfx.device.clone(),
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        vertex_size,
                    ),
                    Buffer::new_uninit(
                        gfx.device.clone(),
                        vk::BufferUsageFlags::INDEX_BUFFER,
                        index_size,
                    ),
                )
            };

            let vertex_align = vertex_buffer.memory_requirements().alignment;
            let index_align = index_buffer.memory_requirements().alignment;

            first_index_align.get_or_insert(index_align);

            extend_align(&mut vertex_buffers, vertex_align.cast());
            extend_align(&mut index_buffers, index_align.cast());

            let vertex_offset = vertex_buffers.len().cast();
            let index_offset = index_buffers.len().cast();

            assert_eq!(vertex_offset % vertex_align, 0);
            assert_eq!(index_offset % index_align, 0);

            for vertex in vertexes {
                vertex_buffers.extend(vertex.as_u8_slice());
            }

            for index in indices {
                index_buffers.extend(index.to_ne_bytes());
            }

            MeshInfo {
                vertex_buffer,
                index_buffer,
                material: materials[primitive.material().index().unwrap()].clone(),
                vertex_offset,
                index_offset,
            }
        })
        .collect::<Box<[_]>>();

    extend_align(&mut vertex_buffers, first_index_align.unwrap().cast());

    let vertex_byte_length = vertex_buffers.len();
    let index_byte_length = index_buffers.len();

    let buffer = Buffer::new_staged_with(
        &gfx.device,
        cmd_buf,
        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
        |mapped_memory: &mut [u8]| {
            mapped_memory[0..vertex_byte_length].copy_from_slice(&vertex_buffers);

            mapped_memory[vertex_byte_length..].copy_from_slice(&index_buffers);
        },
        vertex_byte_length + index_byte_length,
    );

    let make_primitive = |mut info: MeshInfo| {
        let buffer_memory = buffer.memory.as_ref().unwrap().memory.clone();
        let index_offset: vk::DeviceSize = vertex_byte_length.cast();

        unsafe {
            info.vertex_buffer
                .bind_memory(BufferMemory::new(buffer_memory.clone(), info.vertex_offset));
            info.index_buffer.bind_memory(BufferMemory::new(
                buffer_memory,
                index_offset + info.index_offset,
            ));
        }

        Primitive::new_raw(
            info.vertex_buffer.into(),
            info.index_buffer.into(),
            info.material,
        )
    };

    let mesh = mesh_info.into_iter().map(make_primitive).collect();

    Node::empty().mesh(Mesh::new(mesh).into())
}
