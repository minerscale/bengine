use std::sync::Arc;

use crate::renderer::{
    descriptors::{DescriptorPool, DescriptorSet, DescriptorSetLayout},
    device::Device,
    image::Image,
    sampler::Sampler,
};

pub const MAX_TEXTURES: u32 = 40;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct MaterialProperties {
    pub alpha_cutoff: f32,
    pub is_water: u32,
}

impl Default for MaterialProperties {
    fn default() -> Self {
        Self {
            alpha_cutoff: 0.0,
            is_water: 0,
        }
    }
}

#[derive(Debug)]
pub struct Material {
    pub descriptor_set: DescriptorSet,
    pub properties: MaterialProperties,
    pub name: Option<String>,
}

impl Material {
    pub fn new(
        device: &Device,
        image: Arc<Image>,
        sampler: Arc<Sampler>,
        name: Option<String>,
        properties: MaterialProperties,
        descriptor_pool: &DescriptorPool,
        descriptor_set_layout: &DescriptorSetLayout,
    ) -> Self {
        let mut descriptor_set = descriptor_pool.create_descriptor_set(descriptor_set_layout);

        descriptor_set.bind_texture(device, 0, image, sampler);

        Self {
            descriptor_set,
            properties,
            name,
        }
    }
}
