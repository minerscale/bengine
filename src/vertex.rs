use std::{marker::PhantomData, mem::offset_of};

use ash::vk::{self, TaggedStructure};
use obj::FromRawVertex;
use ultraviolet::{Vec2, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub tex_coord: Vec2,
}

impl<I: Copy + num_traits::cast::FromPrimitive> FromRawVertex<I> for Vertex {
    fn process(
        vertices: Vec<(f32, f32, f32, f32)>,
        normals: Vec<(f32, f32, f32)>,
        tex_coords: Vec<(f32, f32, f32)>,
        polygons: Vec<obj::raw::object::Polygon>,
    ) -> obj::ObjResult<(Vec<Self>, Vec<I>)> {
        let (v, i) = obj::TexturedVertex::process(vertices, normals, tex_coords, polygons)?;

        Ok((
            v.iter()
                .map(|v| Self {
                    pos: Vec3::from(v.position),
                    normal: Vec3::from(v.normal),
                    tex_coord: Vec2::new(v.texture[0], v.texture[1]),
                })
                .collect::<Vec<_>>(),
            i,
        ))
    }
}

impl Vertex {
    pub const fn new(pos: Vec3, normal: Vec3, tex_coord: Vec2) -> Self {
        Self {
            pos,
            normal,
            tex_coord,
        }
    }

    pub const fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: size_of::<Self>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }

    pub const fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 3] {
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
            vk::VertexInputAttributeDescription {
                location: 2,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Self, tex_coord) as u32,
            },
        ]
    }

    pub const fn get_input_state_create_info()
    -> &'static vk::PipelineVertexInputStateCreateInfo<'static> {
        const BINDING_DESCRIPTION: &[vk::VertexInputBindingDescription] =
            &[Vertex::get_binding_description()];
        const ATTRIBUTE_DESCRIPTIONS: &[vk::VertexInputAttributeDescription] =
            &Vertex::get_attribute_descriptions();

        const INPUT_STATE_CREATE_INFO: &vk::PipelineVertexInputStateCreateInfo =
            &vk::PipelineVertexInputStateCreateInfo {
                s_type: vk::PipelineVertexInputStateCreateInfo::STRUCTURE_TYPE,
                p_next: ::core::ptr::null(),
                flags: vk::PipelineVertexInputStateCreateFlags::empty(),
                vertex_binding_description_count: BINDING_DESCRIPTION.len() as u32,
                p_vertex_binding_descriptions: BINDING_DESCRIPTION.as_ptr(),
                vertex_attribute_description_count: ATTRIBUTE_DESCRIPTIONS.len() as u32,
                p_vertex_attribute_descriptions: ATTRIBUTE_DESCRIPTIONS.as_ptr(),
                _marker: PhantomData,
            };

        INPUT_STATE_CREATE_INFO
    }
}
