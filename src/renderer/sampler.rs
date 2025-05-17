use std::rc::Rc;

use ash::vk;
use log::info;

pub struct Sampler {
    pub sampler: vk::Sampler,
    device: Rc<ash::Device>,
}

impl Sampler {
    pub fn new(
        instance: &ash::Instance,
        device: Rc<ash::Device>,
        physical_device: vk::PhysicalDevice,
    ) -> Self {
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .anisotropy_enable(true)
            .max_anisotropy(unsafe {
                instance
                    .get_physical_device_properties(physical_device)
                    .limits
                    .max_sampler_anisotropy
            })
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .max_lod(0.0);

        let sampler = unsafe { device.create_sampler(&sampler_info, None).unwrap() };

        Self { sampler, device }
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        info!("dropped sampler");
        unsafe { self.device.destroy_sampler(self.sampler, None) };
    }
}
