use std::rc::Rc;

use crate::renderer::{
    descriptors::{DescriptorPool, DescriptorSet, DescriptorSetLayout},
    device::Device,
    image::Image,
    sampler::Sampler,
};

pub const MAX_TEXTURES: u32 = 3;

#[derive(Debug)]
pub struct Texture {
    pub descriptor_set: DescriptorSet,
}

impl Texture {
    pub fn new(
        device: &Device,
        image: Rc<Image>,
        sampler: Rc<Sampler>,
        descriptor_pool: &DescriptorPool,
        descriptor_set_layout: &DescriptorSetLayout,
    ) -> Self {
        let mut descriptor_set = descriptor_pool.create_descriptor_set(descriptor_set_layout);

        descriptor_set.bind_texture(device, 1, image.clone(), sampler.clone());

        Self { descriptor_set }
    }
}
