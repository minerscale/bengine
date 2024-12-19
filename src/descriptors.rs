use std::rc::Rc;

use ash::vk;
use log::info;

use crate::renderer::MAX_FRAMES_IN_FLIGHT;

#[derive(Clone)]
pub struct DescriptorSetLayout {
    pub layout: vk::DescriptorSetLayout,
    device: Rc<ash::Device>,
}

impl DescriptorSetLayout {
    pub fn new(device: Rc<ash::Device>) -> Self {
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);

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
                .descriptor_count(MAX_FRAMES_IN_FLIGHT.try_into().unwrap()),
        ];

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_FRAMES_IN_FLIGHT.try_into().unwrap());

        let pool = unsafe { device.create_descriptor_pool(&pool_info, None).unwrap() };

        Self { pool, device }
    }

    pub fn create_descriptor_sets(
        &self,
        set_layouts: &[vk::DescriptorSetLayout],
    ) -> Vec<vk::DescriptorSet> {
        let allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.pool)
            .set_layouts(&set_layouts);

        unsafe {
            self.device
                .allocate_descriptor_sets(&allocate_info)
                .unwrap()
        }
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        info!("dropped descriptor pool");

        unsafe { self.device.destroy_descriptor_pool(self.pool, None) };
    }
}
