use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Cursor},
    path::Path,
    sync::Arc,
};

use ash::vk;
use gltf::Gltf;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use ultraviolet::Vec3;

use crate::{
    mesh::{Mesh, Primitive},
    node::Node,
    renderer::{
        Renderer,
        buffer::Buffer,
        command_buffer::OneTimeSubmitCommandBuffer,
        image::Image,
        material::{Material, MaterialProperties},
        sampler::Sampler,
    },
    shader_pipelines::MATERIAL_LAYOUT,
    vertex::Vertex,
};

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

    fn get_uri(view: gltf::buffer::View) -> String {
        view.index().to_string() + &view.offset().to_string()
    }

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
                        get_uri(view),
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
                gltf::image::Source::View { view, mime_type: _ } => &images[&get_uri(view)],
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

    let mut vertex_buffers: Vec<Vertex> = Vec::new();
    let mut index_buffers: Vec<u32> = Vec::new();

    struct MeshInfo {
        vertex_idx: usize,
        vertex_size: usize,
        index_idx: usize,
        index_size: usize,
        material: Arc<Material>,
    }

    let mut mesh_info: Vec<MeshInfo> = Vec::new();

    for primitive in document.meshes().next().unwrap().primitives() {
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

        let vertex_idx = vertex_buffers.len();

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
        vertex_buffers.extend(vertexes);

        let index_idx = index_buffers.len();
        let indices = reader.read_indices().unwrap().into_u32();
        let index_size = indices.len();
        index_buffers.extend(indices);

        mesh_info.push(MeshInfo {
            vertex_idx,
            vertex_size,
            index_idx,
            index_size,
            material: materials[primitive.material().index().unwrap()].clone(),
        });
    }

    let vertex_byte_length = vertex_buffers.len() * size_of::<Vertex>();
    let index_byte_length = index_buffers.len() * size_of::<u32>();

    let buffer = Buffer::new_staged_with(
        &gfx.device,
        cmd_buf,
        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
        |mapped_memory: &mut [u8]| {
            mapped_memory[0..vertex_byte_length].copy_from_slice(unsafe {
                std::slice::from_raw_parts(vertex_buffers.as_ptr() as *const u8, vertex_byte_length)
            });

            mapped_memory[vertex_byte_length..].copy_from_slice(unsafe {
                std::slice::from_raw_parts(index_buffers.as_ptr() as *const u8, index_byte_length)
            });
        },
        vertex_byte_length + index_byte_length,
    );

    let make_primitive = |info: MeshInfo| {
        Primitive::new_raw(
            Buffer::new_with_memory(
                vk::BufferUsageFlags::VERTEX_BUFFER,
                (
                    buffer.memory.0.clone(),
                    (info.vertex_idx * size_of::<Vertex>()).try_into().unwrap(),
                ),
                gfx.device.clone(),
                info.vertex_size,
            )
            .into(),
            Buffer::new_with_memory(
                vk::BufferUsageFlags::INDEX_BUFFER,
                (
                    buffer.memory.0.clone(),
                    (info.index_idx * size_of::<u32>() + vertex_byte_length)
                        .try_into()
                        .unwrap(),
                ),
                gfx.device.clone(),
                info.index_size,
            )
            .into(),
            info.material,
        )
    };

    let mesh = mesh_info.into_iter().map(make_primitive).collect();

    Node::empty().mesh(Mesh::new(mesh).into())
}
