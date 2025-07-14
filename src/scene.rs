use std::{io::Cursor, rc::Rc};

use ash::vk;
use rapier3d::{
    math::AngVector,
    na::vector,
    prelude::{ColliderBuilder, RigidBodyBuilder},
};
use ultraviolet::Vec3;

use crate::{
    mesh::{Mesh, collider_from_obj},
    node::{GameTree, Node},
    physics::Physics,
    renderer::{
        Renderer, command_buffer::OneTimeSubmitCommandBuffer, image::Image, sampler::Sampler,
        texture::Texture,
    },
};

fn scene(
    gfx: &Renderer,
    cmd_buf: &mut OneTimeSubmitCommandBuffer,
    physics: &mut Physics,
) -> GameTree {
    macro_rules! image {
        ($filename:literal) => {
            Rc::new(Image::from_bytes(
                &gfx.instance,
                gfx.device.physical_device,
                &gfx.device.device,
                cmd_buf,
                include_bytes!($filename),
            ))
        };
    }

    macro_rules! mesh {
        ($filename:literal, $scale:expr) => {
            Rc::new(Mesh::new(
                &gfx.instance,
                gfx.device.physical_device,
                gfx.device.device.clone(),
                Cursor::new(include_bytes!($filename)),
                cmd_buf,
                $scale,
            ))
        };
    }

    macro_rules! texture {
        ($sampler:expr, $texture:expr) => {
            Rc::new(Texture::new(
                &gfx.device,
                $texture.clone(),
                $sampler.clone(),
                &gfx.descriptor_pool,
                &gfx.texture_layout,
            ))
        };
    }

    macro_rules! raw_obj {
        ($filename:literal) => {
            &obj::raw::parse_obj(&include_bytes!($filename)[..]).unwrap()
        };
    }

    let sampler = Rc::new(Sampler::new(
        &gfx.instance,
        gfx.device.device.clone(),
        gfx.device.physical_device,
        vk::SamplerAddressMode::REPEAT,
        true,
    ));

    let grid = texture!(sampler, image!("../textures/grid.png"));

    let middle_grey = texture!(sampler, image!("../test-scene/middle-grey.png"));

    let cube_2_scale = Vec3::new(1.0, 0.4, 1.0);

    let root_node = Node::empty()
        .child(
            Node::empty()
                .model(
                    mesh!("../test-scene/icosehedron.obj", None),
                    middle_grey.clone(),
                )
                .rigid_body(
                    physics,
                    ColliderBuilder::new(collider_from_obj(
                        raw_obj!("../test-scene/icosehedron.obj"),
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
                .model(
                    mesh!("../test-scene/cube.obj", Some(cube_2_scale)),
                    middle_grey.clone(),
                )
                .rigid_body(
                    physics,
                    ColliderBuilder::new(collider_from_obj(
                        raw_obj!("../test-scene/cube.obj"),
                        Some(cube_2_scale),
                        None,
                    )),
                    RigidBodyBuilder::dynamic().translation(vector![0.0, 5.0, 0.0]),
                ),
        )
        .child(
            Node::empty()
                .model(mesh!("../test-objects/ground-plane.obj", None), grid)
                .collider(
                    physics,
                    ColliderBuilder::cuboid(100.0, 0.1, 100.0).translation(vector![0.0, -0.1, 0.0]),
                ),
        );

    GameTree::new(root_node)
}

pub fn create_scene(gfx: &Renderer, physics: &mut Physics) -> GameTree {
    gfx.command_pool
        .one_time_submit(gfx.device.graphics_queue, |cmd_buf| {
            scene(gfx, cmd_buf, physics)
        })
}
