use std::{io::Cursor, sync::Arc};

use ash::vk;
use rapier3d::{
    math::AngVector,
    na::vector,
    prelude::{ColliderBuilder, RigidBodyBuilder},
};
use ultraviolet::Vec3;

use crate::{
    gltf::load_gltf,
    mesh::{Mesh, Primitive, collider_from_obj},
    node::Node,
    physics::Physics,
    renderer::{
        Renderer,
        command_buffer::OneTimeSubmitCommandBuffer,
        image::Image,
        material::{Material, MaterialProperties},
        sampler::Sampler,
    },
    shader_pipelines::MATERIAL_LAYOUT,
};

fn scene(
    gfx: &Renderer,
    cmd_buf: &mut OneTimeSubmitCommandBuffer,
    physics: &mut Physics,
) -> Vec<Node> {
    macro_rules! image {
        ($filename:literal) => {
            Image::from_bytes(&gfx.device, cmd_buf, include_bytes!($filename))
        };
    }

    macro_rules! mesh {
        ($filename:literal, $material:expr, $scale:expr) => {
            Arc::new(Mesh::new(Box::new([Primitive::from_obj(
                &gfx.device,
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
            gfx.device.clone(),
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
            Err(include_bytes!("../test-objects/Sponza.glb")),
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
