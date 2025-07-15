use std::{io::BufRead, rc::Rc};

use ash::vk;
use obj::raw::RawObj;
use obj::{Obj, load_obj};
use rapier3d::na;
use rapier3d::prelude::ColliderShape;
use ultraviolet::Vec3;

use crate::renderer::{buffer::Buffer, command_buffer::ActiveCommandBuffer, material::Material};

use crate::vertex::Vertex;

#[derive(Debug)]
pub struct Mesh {
    pub primitives: Vec<Primitive>,
}

impl<'a> IntoIterator for &'a Mesh {
    type Item = &'a Primitive;
    type IntoIter = std::slice::Iter<'a, Primitive>;

    fn into_iter(self) -> Self::IntoIter {
        self.primitives.iter()
    }
}

impl Mesh {
    pub fn new(primitives: Vec<Primitive>) -> Self {
        Self { primitives }
    }
}

#[derive(Debug)]
pub struct Primitive {
    pub vertex_buffer: Rc<Buffer<Vertex>>,
    pub index_buffer: Rc<Buffer<u32>>,
    pub material: Rc<Material>,
}

impl Primitive {
    pub fn new<C: ActiveCommandBuffer>(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: Rc<ash::Device>,
        vertex_buffer: &[Vertex],
        index_buffer: &[u32],
        material: Rc<Material>,
        cmd_buf: &mut C,
    ) -> Self {
        let vertex_buffer = Buffer::new_staged(
            instance,
            device.clone(),
            physical_device,
            cmd_buf,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vertex_buffer,
        );

        let index_buffer = Buffer::new_staged(
            instance,
            device,
            physical_device,
            cmd_buf,
            vk::BufferUsageFlags::INDEX_BUFFER,
            &index_buffer,
        );

        Self {
            vertex_buffer,
            index_buffer,
            material,
        }
    }

    pub fn from_obj<T: BufRead, C: ActiveCommandBuffer>(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: Rc<ash::Device>,
        file: T,
        cmd_buf: &mut C,
        material: Rc<Material>,
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
            material,
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
