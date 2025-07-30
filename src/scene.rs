use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Cursor},
    path::Path,
    sync::Arc,
};

use ash::vk;
use gltf::Gltf;
use rapier3d::{
    math::AngVector,
    na::vector,
    prelude::{ColliderBuilder, RigidBodyBuilder},
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use ultraviolet::Vec3;

use crate::{
    mesh::{Mesh, Primitive, collider_from_obj},
    node::Node,
    physics::Physics,
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

fn load_gltf(
    gfx: &Renderer,
    cmd_buf: &mut OneTimeSubmitCommandBuffer,
    filename: &str,
    scale: f32,
) -> Node {
    let root = Path::new(filename)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let gltf = Gltf::from_slice(include_bytes!("../test-objects/Sponza.glb")).unwrap();
    
    //let gltf = gltf::Glb::from_reader(BufReader::new(File::open("test-objects/Sponza.gltf").unwrap())).unwrap();

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

                    (get_uri(view), image::ImageReader::new(Cursor::new(buffer))
                        .with_guessed_format()
                        .unwrap()
                        .decode()
                        .unwrap())
                }
                gltf::image::Source::Uri { uri, mime_type: _ } => (uri.to_owned(), image::ImageReader::new(
                    BufReader::new(File::open(root.join(Path::new(uri))).unwrap()),
                )
                .with_guessed_format()
                .unwrap()
                .decode()
                .unwrap()),
            };

            (
                uri,
                image,
            )
        })
        .collect::<Box<_>>()
        .into_iter()
        .map(|(uri, image)| {
            (
                uri,
                Image::from_image(
                    &gfx.instance,
                    gfx.device.physical_device,
                    &gfx.device.device,
                    cmd_buf,
                    image,
                    true,
                ),
            )
        })
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
                gltf::image::Source::View {
                    view,
                    mime_type: _,
                } => {
                    &images[&get_uri(view)]
                },
                gltf::image::Source::Uri { uri, mime_type: _ } => &images[uri],
            };

            let properties = MaterialProperties {
                alpha_cutoff: material.alpha_cutoff().unwrap_or(0.0),
            };

            Arc::new(Material::new(
                &gfx.device,
                image.clone(),
                Arc::new(Sampler::new(
                    &gfx.instance,
                    gfx.device.device.clone(),
                    gfx.device.physical_device,
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

    let mut mesh_info: Vec<((usize, usize), (usize, usize), Arc<Material>)> = Vec::new();

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

        mesh_info.push((
            (vertex_idx, vertex_size),
            (index_idx, index_size),
            materials[primitive.material().index().unwrap()].clone(),
        ));
    }

    let vertex_byte_length = vertex_buffers.len() * size_of::<Vertex>();
    let index_byte_length = index_buffers.len() * size_of::<u32>();

    let buffer = Buffer::new_staged_with(
        &gfx.instance,
        gfx.device.device.clone(),
        gfx.device.physical_device,
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

    let make_primitive = |((vertex_idx, vertex_size), (index_idx, index_size), material): (
        (usize, usize),
        (usize, usize),
        Arc<Material>,
    )| {
        Primitive::new_raw(
            Buffer::new_with_memory(
                vk::BufferUsageFlags::VERTEX_BUFFER,
                (
                    buffer.memory.0.clone(),
                    (vertex_idx * size_of::<Vertex>()).try_into().unwrap(),
                ),
                gfx.device.device.clone(),
                vertex_size,
            )
            .into(),
            Buffer::new_with_memory(
                vk::BufferUsageFlags::INDEX_BUFFER,
                (
                    buffer.memory.0.clone(),
                    (index_idx * size_of::<u32>() + vertex_byte_length)
                        .try_into()
                        .unwrap(),
                ),
                gfx.device.device.clone(),
                index_size,
            )
            .into(),
            material,
        )
    };

    let mesh = mesh_info.into_iter().map(make_primitive).collect();

    Node::empty().mesh(Mesh::new(mesh).into())
}

fn scene(
    gfx: &Renderer,
    cmd_buf: &mut OneTimeSubmitCommandBuffer,
    physics: &mut Physics,
) -> Vec<Node> {
    macro_rules! image {
        ($filename:literal) => {
            Image::from_bytes(
                &gfx.instance,
                gfx.device.physical_device,
                &gfx.device.device,
                cmd_buf,
                include_bytes!($filename),
            )
        };
    }

    macro_rules! mesh {
        ($filename:literal, $material:expr, $scale:expr) => {
            Arc::new(Mesh::new(Box::new([Primitive::from_obj(
                &gfx.instance,
                gfx.device.physical_device,
                gfx.device.device.clone(),
                Cursor::new(include_bytes!($filename)),
                cmd_buf,
                $material,
                $scale,
            )])))
        };
    }

    macro_rules! texture {
        ($sampler:expr, $texture:expr) => {
            Arc::new(Material::new(
                &gfx.device,
                $texture.clone(),
                $sampler.clone(),
                MaterialProperties::default(),
                &gfx.descriptor_pool,
                &gfx.descriptor_set_layouts[MATERIAL_LAYOUT],
            ))
        };
    }

    macro_rules! raw_obj {
        ($filename:literal) => {
            &obj::raw::parse_obj(&include_bytes!($filename)[..]).unwrap()
        };
    }

    let sampler = |mip_levels: u32| {
        Arc::new(Sampler::new(
            &gfx.instance,
            gfx.device.device.clone(),
            gfx.device.physical_device,
            vk::SamplerAddressMode::REPEAT,
            vk::Filter::LINEAR,
            vk::Filter::LINEAR,
            true,
            Some((vk::SamplerMipmapMode::LINEAR, mip_levels)),
        ))
    };

    let grid_image = image!("../test-objects/grid.png");
    let grid = texture!(sampler(grid_image.mip_levels), grid_image);

    let middle_grey_image = image!("../test-objects/middle-grey.png");
    let middle_grey = texture!(sampler(middle_grey_image.mip_levels), middle_grey_image);

    let cube_2_scale = Vec3::new(1.0, 0.4, 1.0);

    let scene = vec![
        Node::empty()
            .mesh(mesh!(
                "../test-objects/icosehedron.obj",
                middle_grey.clone(),
                None
            ))
            .rigid_body(
                physics,
                ColliderBuilder::new(collider_from_obj(
                    raw_obj!("../test-objects/icosehedron.obj"),
                    None,
                    None,
                )),
                RigidBodyBuilder::dynamic()
                    .translation(vector![3.0, 10.0, 0.0])
                    .rotation(AngVector::new(0.5, 1.2, 3.1)),
            ),
        Node::empty()
            .mesh(mesh!(
                "../test-objects/cube.obj",
                middle_grey,
                Some(cube_2_scale)
            ))
            .rigid_body(
                physics,
                ColliderBuilder::new(collider_from_obj(
                    raw_obj!("../test-objects/cube.obj"),
                    Some(cube_2_scale),
                    None,
                )),
                RigidBodyBuilder::dynamic().translation(vector![0.0, 5.0, 0.0]),
            ),
        Node::empty()
            .mesh(mesh!("../test-objects/ground-plane.obj", grid, None))
            .collider(
                physics,
                ColliderBuilder::cuboid(100.0, 0.1, 100.0).translation(vector![0.0, -0.1, 0.0]),
            ),
        load_gltf(
            gfx,
            cmd_buf,
            "glTF-Sample-Assets/Models/Sponza/glTF/Sponza.gltf",
            0.025,
        ),
    ];

    scene
}

pub fn create_scene(gfx: &Renderer, physics: &mut Physics) -> Vec<Node> {
    gfx.command_pool
        .one_time_submit(gfx.device.graphics_queue, |cmd_buf| {
            scene(gfx, cmd_buf, physics)
        })
}
