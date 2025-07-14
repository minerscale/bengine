use std::{io::BufRead, rc::Rc};

use ash::vk;
use obj::raw::RawObj;
use obj::{Obj, load_obj};
use rapier3d::na;
use rapier3d::prelude::ColliderShape;
use ultraviolet::Vec3;

use crate::renderer::{buffer::Buffer, command_buffer::ActiveCommandBuffer};

use crate::vertex::Vertex;

#[derive(Debug)]
pub struct Mesh {
    pub vertex_buffer: Buffer<Vertex>,
    pub index_buffer: Buffer<u32>,
}

impl Mesh {
    pub fn new<T: BufRead, C: ActiveCommandBuffer>(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: Rc<ash::Device>,
        file: T,
        cmd_buf: &mut C,
        scale: Option<Vec3>,
    ) -> Self {
        let mut mesh: Obj<Vertex, u32> = load_obj(file).unwrap();

        if let Some(scale) = scale {
            for vertex in &mut mesh.vertices {
                vertex.pos *= scale;
            }
        }

        let vertex_buffer = Buffer::new_staged(
            instance,
            device.clone(),
            physical_device,
            cmd_buf,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            &mesh.vertices,
        );

        let index_buffer = Buffer::new_staged(
            instance,
            device,
            physical_device,
            cmd_buf,
            vk::BufferUsageFlags::INDEX_BUFFER,
            &mesh.indices,
        );

        Self {
            vertex_buffer,
            index_buffer,
        }
    }
}

pub fn collider_from_obj(
    mesh: &RawObj,
    scale: Option<Vec3>,
    transform: Option<Vec3>,
) -> ColliderShape {
    type Point = na::Point<f32, 3>;

    let vertices: Box<[Point]> = mesh
        .positions
        .iter()
        .map(|v| {
            Point::from_slice(
                transform
                    .unwrap_or_else(|| {
                        Vec3::zero() + scale.unwrap_or_else(Vec3::one) * Vec3::new(v.0, v.1, v.2)
                    })
                    .as_array(),
            )
        })
        .collect();

    ColliderShape::convex_hull(&vertices).unwrap()
}
