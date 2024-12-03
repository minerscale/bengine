use std::mem::offset_of;

use ash::vk;
use geometric_algebra::vector::Vector;
use obj::FromRawVertex;

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: Vector<f32>,
    pub normal: Vector<f32>,
}

impl<I: Copy + num_traits::cast::FromPrimitive> FromRawVertex<I> for Vertex {
    fn process(
        vertices: Vec<(f32, f32, f32, f32)>,
        normals: Vec<(f32, f32, f32)>,
        tex_coords: Vec<(f32, f32, f32)>,
        polygons: Vec<obj::raw::object::Polygon>,
    ) -> obj::ObjResult<(Vec<Self>, Vec<I>)> {
        let (v, i) = obj::Vertex::process(vertices, normals, tex_coords, polygons)?;

        Ok((
            v.iter()
                .map(|v| Vertex {
                    pos: Vector::from_slice(v.position),
                    normal: Vector::from_slice(v.normal),
                })
                .collect::<Vec<_>>(),
            i,
        ))
    }
}

impl Vertex {
    pub const fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }

    pub const fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Self, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Self, normal) as u32,
            },
        ]
    }
}
