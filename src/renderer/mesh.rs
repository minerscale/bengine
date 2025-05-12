use std::{io::BufRead, rc::Rc};

use ash::vk;
use obj::{Obj, load_obj};
use ultraviolet::Vec3;

use crate::renderer::{buffer::Buffer, command_buffer::ActiveCommandBuffer, vertex::Vertex};

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

        scale.map(|scale| {
            for vertex in &mut mesh.vertices {
                vertex.pos *= scale;
            }
        });

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
            device.clone(),
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
