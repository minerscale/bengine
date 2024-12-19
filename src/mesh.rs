use std::io::BufRead;

use ash::vk;
use obj::{load_obj, Obj};

use crate::{
    buffer::Buffer, command_buffer::ActiveCommandBuffer, renderer::Renderer, vertex::Vertex,
};

#[derive(Debug)]
pub struct Mesh {
    pub vertex_buffer: Buffer<Vertex>,
    pub index_buffer: Buffer<u32>,
}

impl Mesh {
    pub fn new<T: BufRead, C: ActiveCommandBuffer>(
        file: T,
        gfx: &Renderer,
        cmd_buf: &mut C,
    ) -> Self {
        let teapot: Obj<Vertex, u32> = load_obj(file).unwrap();

        let vertex_buffer = Buffer::new_staged(
            &gfx.instance,
            gfx.device.device.clone(),
            gfx.device.physical_device,
            cmd_buf,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            &teapot.vertices,
        );

        let index_buffer = Buffer::new_staged(
            &gfx.instance,
            gfx.device.device.clone(),
            gfx.device.physical_device,
            cmd_buf,
            vk::BufferUsageFlags::INDEX_BUFFER,
            &teapot.indices,
        );

        Self {
            vertex_buffer,
            index_buffer,
        }
    }
}
