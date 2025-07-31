use std::{io::BufRead, mem::offset_of, ptr::addr_of, sync::Arc};

use ash::vk;
use easy_cast::Cast;
use obj::{Obj, load_obj, raw::RawObj};
use rapier3d::{na, prelude::ColliderShape};
use ultraviolet::{Isometry3, Vec3};

use crate::{
    renderer::{
        buffer::Buffer,
        command_buffer::ActiveCommandBuffer,
        device::Device,
        material::{Material, MaterialProperties},
        pipeline::Pipeline,
    },
    shader_pipelines::PushConstants,
    vertex::Vertex,
};

#[derive(Debug)]
pub struct Mesh {
    pub primitives: Box<[Primitive]>,
}

impl<'a> IntoIterator for &'a Mesh {
    type Item = &'a Primitive;
    type IntoIter = std::slice::Iter<'a, Primitive>;

    fn into_iter(self) -> Self::IntoIter {
        self.primitives.iter()
    }
}

impl Mesh {
    pub fn new(primitives: Box<[Primitive]>) -> Self {
        Self { primitives }
    }

    pub fn draw(
        &self,
        device: &ash::Device,
        cmd_buf: vk::CommandBuffer,
        pipeline: &Pipeline,
        modelview: Isometry3,
    ) {
        unsafe {
            device.cmd_push_constants(
                cmd_buf,
                pipeline.pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                offset_of!(PushConstants, model_transform)
                    .try_into()
                    .unwrap(),
                std::slice::from_raw_parts(
                    addr_of!(modelview).cast::<u8>(),
                    std::mem::size_of::<Isometry3>(),
                ),
            );

            for primitive in self {
                let descriptor_sets = [*primitive.material.descriptor_set];

                device.cmd_push_constants(
                    cmd_buf,
                    pipeline.pipeline_layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    offset_of!(PushConstants, material_properties)
                        .try_into()
                        .unwrap(),
                    std::slice::from_raw_parts(
                        addr_of!(primitive.material.properties).cast::<u8>(),
                        std::mem::size_of::<MaterialProperties>(),
                    ),
                );

                device.cmd_bind_descriptor_sets(
                    cmd_buf,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline.pipeline_layout,
                    1,
                    &descriptor_sets,
                    &[],
                );

                let vertex_buffers = [primitive.vertex_buffer.buffer];
                let offsets = [vk::DeviceSize::from(0u64)];

                device.cmd_bind_vertex_buffers(cmd_buf, 0, &vertex_buffers, &offsets);
                device.cmd_bind_index_buffer(
                    cmd_buf,
                    primitive.index_buffer.buffer,
                    0,
                    vk::IndexType::UINT32,
                );

                device.cmd_draw_indexed(cmd_buf, primitive.index_buffer.len().cast(), 1, 0, 0, 0);
            }
        }
    }
}

#[derive(Debug)]
pub struct Primitive {
    pub vertex_buffer: Arc<Buffer<Vertex>>,
    pub index_buffer: Arc<Buffer<u32>>,
    pub material: Arc<Material>,
}

impl Primitive {
    pub fn new_raw(
        vertex_buffer: Arc<Buffer<Vertex>>,
        index_buffer: Arc<Buffer<u32>>,
        material: Arc<Material>,
    ) -> Self {
        Self {
            vertex_buffer,
            index_buffer,
            material,
        }
    }

    pub fn new<C: ActiveCommandBuffer>(
        device: &Arc<Device>,
        vertex_buffer: &[Vertex],
        index_buffer: &[u32],
        material: Arc<Material>,
        cmd_buf: &mut C,
    ) -> Self {
        let vertex_buffer = Buffer::new_staged(
            device,
            cmd_buf,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vertex_buffer,
        );

        let index_buffer = Buffer::new_staged(
            device,
            cmd_buf,
            vk::BufferUsageFlags::INDEX_BUFFER,
            index_buffer,
        );

        Self {
            vertex_buffer,
            index_buffer,
            material,
        }
    }

    pub fn from_obj<T: BufRead, C: ActiveCommandBuffer>(
        device: &Arc<Device>,
        file: T,
        cmd_buf: &mut C,
        material: Arc<Material>,
        scale: Option<Vec3>,
    ) -> Self {
        let mut mesh: Obj<Vertex, u32> = load_obj(file).unwrap();

        if let Some(scale) = scale {
            for vertex in &mut mesh.vertices {
                vertex.pos *= scale;
            }
        }

        let vertex_buffer = Buffer::new_staged(
            device,
            cmd_buf,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            &mesh.vertices,
        );

        let index_buffer = Buffer::new_staged(
            device,
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
