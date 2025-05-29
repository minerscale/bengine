use std::rc::Rc;

use ash::vk;
use log::debug;

pub struct Sampler {
    pub sampler: vk::Sampler,
    device: Rc<ash::Device>,
}

impl Sampler {
    pub fn new(
        instance: &ash::Instance,
        device: Rc<ash::Device>,
        physical_device: vk::PhysicalDevice,
        address_mode: vk::SamplerAddressMode,
        anisotropy_enable: bool,
    ) -> Self {
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(address_mode)
            .address_mode_v(address_mode)
            .address_mode_w(address_mode)
            .anisotropy_enable(anisotropy_enable)
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
        debug!("dropped sampler");
        unsafe { self.device.destroy_sampler(self.sampler, None) };
    }
}
