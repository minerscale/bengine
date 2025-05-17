use std::{ops::Deref, rc::Rc};

use ash::vk;
use log::info;

use crate::renderer::{
    MAX_FRAMES_IN_FLIGHT, buffer::Buffer, image::Image, sampler::Sampler, texture::MAX_TEXTURES,
};

#[derive(Clone)]
pub struct DescriptorSetLayout {
    pub layout: vk::DescriptorSetLayout,
    device: Rc<ash::Device>,
}

#[derive(Debug)]
pub struct DescriptorSet {
    pub descriptor_set: vk::DescriptorSet,
    dependencies: Vec<Rc<dyn std::any::Any>>,
}

impl DescriptorSet {
    pub fn add_dependency(&mut self, dependency: Rc<dyn std::any::Any>) {
        self.dependencies.push(dependency);
    }
}

impl Deref for DescriptorSet {
    type Target = vk::DescriptorSet;

    fn deref(&self) -> &Self::Target {
        &self.descriptor_set
    }
}

impl DescriptorSet {
    pub fn bind_buffer<T: Copy + 'static>(&mut self, device: &ash::Device, buffer: Rc<Buffer<T>>) {
        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(**buffer)
            .offset(0)
            .range(size_of::<T>().try_into().unwrap())];

        let descriptor_writes = [vk::WriteDescriptorSet::default()
            .dst_set(**self)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .buffer_info(&buffer_info)];

        unsafe {
            device.update_descriptor_sets(&descriptor_writes, &[]);
            self.dependencies.push(buffer);
        };
    }

    pub fn bind_texture(
        &mut self,
        device: &ash::Device,
        binding: u32,
        texture: Rc<Image>,
        sampler: Rc<Sampler>,
    ) {
        let image_info = [vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(texture.view)
            .sampler(sampler.sampler)];

        let descriptor_writes = [vk::WriteDescriptorSet::default()
            .dst_set(**self)
            .dst_binding(binding)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .image_info(&image_info)];

        unsafe {
            device.update_descriptor_sets(&descriptor_writes, &[]);
            self.dependencies.push(texture);
            self.dependencies.push(sampler);
        };
    }
}

impl DescriptorSetLayout {
    pub fn new(device: Rc<ash::Device>, bindings: &[vk::DescriptorSetLayoutBinding]) -> Self {
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(bindings);

        let layout = unsafe {
            device
                .create_descriptor_set_layout(&layout_info, None)
                .unwrap()
        };

        Self { layout, device }
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        info!("dropped descriptor set layout");

        unsafe { self.device.destroy_descriptor_set_layout(self.layout, None) };
    }
}

pub struct DescriptorPool {
    pub pool: vk::DescriptorPool,
    device: Rc<ash::Device>,
}

impl DescriptorPool {
    pub fn new(device: Rc<ash::Device>) -> Self {
        let pool_sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(MAX_FRAMES_IN_FLIGHT.try_into().unwrap()),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(MAX_TEXTURES),
        ];

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(u32::try_from(MAX_FRAMES_IN_FLIGHT).unwrap() + MAX_TEXTURES);

        let pool = unsafe { device.create_descriptor_pool(&pool_info, None).unwrap() };

        Self { pool, device }
    }

    pub fn create_descriptor_set(
        &self,
        descriptor_set_layout: &DescriptorSetLayout,
    ) -> DescriptorSet {
        let set_layouts = [descriptor_set_layout.layout];

        let allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.pool)
            .set_layouts(&set_layouts);

        let descriptor_set = *unsafe {
            self.device
                .allocate_descriptor_sets(&allocate_info)
                .unwrap()
        }
        .first()
        .unwrap();

        DescriptorSet {
            descriptor_set,
            dependencies: vec![],
        }
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        info!("dropped descriptor pool");

        unsafe { self.device.destroy_descriptor_pool(self.pool, None) };
    }
}
