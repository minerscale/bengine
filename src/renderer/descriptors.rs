use std::{ops::Deref, sync::Arc};

use ash::vk;
use easy_cast::Cast;
use log::debug;

use crate::renderer::{
    MAX_FRAMES_IN_FLIGHT, buffer::Buffer, device::Device, dtor_entry::DtorEntry, image::Image,
    material::MAX_TEXTURES, sampler::Sampler,
};

#[derive(Clone)]
pub struct DescriptorSetLayout {
    pub layout: vk::DescriptorSetLayout,
    pub descriptor_type: vk::DescriptorType,
    pub binding: u32,
    device: Arc<Device>,
}

#[derive(Debug)]
pub struct DescriptorSet {
    pub descriptor_set: vk::DescriptorSet,
    dependencies: Vec<DtorEntry>,
}

impl DescriptorSet {
    pub fn add_dependency<T: Into<DtorEntry>>(&mut self, dependency: T) {
        self.dependencies.push(dependency.into());
    }
}

impl Deref for DescriptorSet {
    type Target = vk::DescriptorSet;

    fn deref(&self) -> &Self::Target {
        &self.descriptor_set
    }
}

impl DescriptorSet {
    pub fn bind_buffer<T: Copy + Sync + Send + 'static>(
        &mut self,
        device: &Device,
        binding: u32,
        buffer: Arc<Buffer<T>>,
    ) {
        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(**buffer)
            .offset(0)
            .range(size_of::<T>().cast())];

        let descriptor_writes = [vk::WriteDescriptorSet::default()
            .dst_set(**self)
            .dst_binding(binding)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .buffer_info(&buffer_info)];

        unsafe {
            device.update_descriptor_sets(&descriptor_writes, &[]);
            self.add_dependency(buffer);
        };
    }

    pub fn bind_image(&mut self, device: &ash::Device, binding: u32, image: Arc<Image>) {
        let image_info = [vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::GENERAL)
            .image_view(image.view)];

        let descriptor_writes = [vk::WriteDescriptorSet::default()
            .dst_set(**self)
            .dst_binding(binding)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .descriptor_count(1)
            .image_info(&image_info)];

        unsafe {
            device.update_descriptor_sets(&descriptor_writes, &[]);
            self.add_dependency(image);
        };
    }

    pub fn bind_texture(
        &mut self,
        device: &ash::Device,
        binding: u32,
        texture: Arc<Image>,
        sampler: Arc<Sampler>,
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
            self.add_dependency(texture);
            self.add_dependency(sampler);
        };
    }
}

impl DescriptorSetLayout {
    pub fn new(device: Arc<Device>, binding: vk::DescriptorSetLayoutBinding) -> Self {
        let bindings = [binding];

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);

        let layout = unsafe {
            device
                .create_descriptor_set_layout(&layout_info, None)
                .unwrap()
        };

        Self {
            layout,
            descriptor_type: binding.descriptor_type,
            binding: binding.binding,
            device,
        }
    }
}

impl Drop for DescriptorSetLayout {
    fn drop(&mut self) {
        debug!("dropped descriptor set layout");

        unsafe { self.device.destroy_descriptor_set_layout(self.layout, None) };
    }
}

pub struct DescriptorPool {
    pub pool: vk::DescriptorPool,
    device: Arc<Device>,
}

const MAX_STORAGE_IMAGES: u32 = 1;
impl DescriptorPool {
    pub fn new(device: Arc<Device>) -> Self {
        let pool_sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(MAX_FRAMES_IN_FLIGHT.cast()),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(MAX_TEXTURES),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(MAX_STORAGE_IMAGES),
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
        debug!("dropped descriptor pool");

        unsafe { self.device.destroy_descriptor_pool(self.pool, None) };
    }
}
