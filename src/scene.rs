use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Cursor},
    path::Path,
    rc::Rc,
};

use ash::vk;
use gltf::Gltf;
use rapier3d::{
    math::AngVector,
    na::vector,
    prelude::{ColliderBuilder, RigidBodyBuilder},
};
use ultraviolet::Vec3;

use crate::{
    mesh::{Mesh, Primitive, collider_from_obj},
    node::{GameTree, Node},
    physics::Physics,
    renderer::{
        Renderer,
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
    let root = Path::new(filename).parent().unwrap_or(Path::new("."));
    let gltf = Gltf::open(filename).unwrap();
    let buffers = gltf::import_buffers(&gltf.document, Some(root), gltf.blob).unwrap();
    let document = gltf.document;

    let mut image_map = HashMap::new();

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
                    view: _,
                    mime_type: _,
                } => todo!(),
                gltf::image::Source::Uri { uri, mime_type: _ } => {
                    image_map.entry(uri).or_insert_with(|| {
                        Image::from_image(
                            &gfx.instance,
                            gfx.device.physical_device,
                            &gfx.device.device,
                            cmd_buf,
                            image::ImageReader::new(BufReader::new(
                                File::open(root.join(Path::new(uri))).unwrap(),
                            ))
                            .with_guessed_format()
                            .unwrap()
                            .decode()
                            .unwrap(),
                        )
                    })
                }
            };

            let properties = MaterialProperties {
                alpha_cutoff: material.alpha_cutoff().unwrap_or(0.0),
            };

            Rc::new(Material::new(
                &gfx.device,
                image.clone(),
                Rc::new(Sampler::new(
                    &gfx.instance,
                    gfx.device.device.clone(),
                    gfx.device.physical_device,
                    vk::SamplerAddressMode::REPEAT,
                    true,
                    image.mip_levels,
                )),
                properties,
                &gfx.descriptor_pool,
                &gfx.descriptor_set_layouts[MATERIAL_LAYOUT],
            ))
        })
        .collect::<Vec<_>>();

    let meshes = document
        .meshes()
        .map(|mesh| {
            Rc::new(Mesh::new(
                mesh.primitives()
                    .map(|primitive| {
                        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                        let vertex_buffer = reader
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
                            })
                            .collect::<Box<[Vertex]>>();

                        let index_buffer = reader
                            .read_indices()
                            .unwrap()
                            .into_u32()
                            .collect::<Box<[u32]>>();

                        Primitive::new(
                            &gfx.instance,
                            gfx.device.physical_device,
                            gfx.device.device.clone(),
                            &vertex_buffer,
                            &index_buffer,
                            materials[primitive.material().index().unwrap()].clone(),
                            cmd_buf,
                        )
                    })
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Vec<_>>();

    Node::empty().mesh(meshes[0].clone())
}

fn scene(
    gfx: &Renderer,
    cmd_buf: &mut OneTimeSubmitCommandBuffer,
    physics: &mut Physics,
) -> GameTree {
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
            Rc::new(Mesh::new(vec![Primitive::from_obj(
                &gfx.instance,
                gfx.device.physical_device,
                gfx.device.device.clone(),
                Cursor::new(include_bytes!($filename)),
                cmd_buf,
                $material,
                $scale,
            )]))
        };
    }

    macro_rules! texture {
        ($sampler:expr, $texture:expr) => {
            Rc::new(Material::new(
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
        Rc::new(Sampler::new(
            &gfx.instance,
            gfx.device.device.clone(),
            gfx.device.physical_device,
            vk::SamplerAddressMode::REPEAT,
            true,
            mip_levels,
        ))
    };

    let grid_image = image!("../test-objects/grid.png");
    let grid = texture!(sampler(grid_image.mip_levels), grid_image);

    let middle_grey_image = image!("../test-objects/middle-grey.png");
    let middle_grey = texture!(sampler(middle_grey_image.mip_levels), middle_grey_image);

    let cube_2_scale = Vec3::new(1.0, 0.4, 1.0);

    let root_node = Node::empty()
        .child(
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
        )
        .child(
            Node::empty()
                .mesh(mesh!(
                    "../test-objects/cube.obj",
                    middle_grey.clone(),
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
        )
        .child(
            Node::empty()
                .mesh(mesh!("../test-objects/ground-plane.obj", grid, None))
                .collider(
                    physics,
                    ColliderBuilder::cuboid(100.0, 0.1, 100.0).translation(vector![0.0, -0.1, 0.0]),
                ),
        )
        .child(load_gltf(
            gfx,
            cmd_buf,
            "glTF-Sample-Assets/Models/Sponza/glTF/Sponza.gltf",
            0.025,
        ));

    GameTree::new(root_node)
}

pub fn create_scene(gfx: &Renderer, physics: &mut Physics) -> GameTree {
    gfx.command_pool
        .one_time_submit(gfx.device.graphics_queue, |cmd_buf| {
            scene(gfx, cmd_buf, physics)
        })
}
