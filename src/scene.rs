use rapier3d::{
    math::{AngVector, Vector},
    prelude::ColliderBuilder,
};
use ultraviolet::{Isometry3, Rotor3, Vec3};

use crate::{
    gltf::{GltfFile, get_trimesh_from_gltf, load_gltf},
    node::Node,
    physics::Physics,
    renderer::{Renderer, command_buffer::OneTimeSubmitCommandBuffer},
};

fn scene(
    gfx: &Renderer,
    cmd_buf: &mut OneTimeSubmitCommandBuffer,
    physics: &mut Physics,
) -> Vec<Node> {
    let beach_glb = GltfFile::Bytes(include_bytes!("../assets/beach.glb"));

    let beach_rotation = std::f32::consts::FRAC_PI_2;

    let scene = vec![
        Node::new(Isometry3::new(
            Vec3::new(0.0, 0.0, 0.0),
            Rotor3::from_rotation_xz(beach_rotation),
        ))
        .mesh(load_gltf(&gfx, cmd_buf, beach_glb, 1.0).into())
        .collider(
            physics,
            get_trimesh_from_gltf(beach_glb).rotation(AngVector::new(0.0, -beach_rotation, 0.0)),
        ),
        Node::empty().collider(
            physics,
            ColliderBuilder::cuboid(1.0, 20.0, 400.0).translation(Vector::new(16.0, 15.0, 0.0)),
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
