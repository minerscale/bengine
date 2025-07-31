use crate::renderer::Device;
use std::sync::Arc;

use ash::vk;
use log::debug;

pub struct Sampler {
    pub sampler: vk::Sampler,
    device: Arc<Device>,
}

impl Sampler {
    pub fn new(
        device: Arc<Device>,
        address_mode: vk::SamplerAddressMode,
        mag_filter: vk::Filter,
        min_filter: vk::Filter,
        anisotropy_enable: bool,
        mipmap_info: Option<(vk::SamplerMipmapMode, u32)>,
    ) -> Self {
        let mut sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(mag_filter)
            .min_filter(min_filter)
            .address_mode_u(address_mode)
            .address_mode_v(address_mode)
            .address_mode_w(address_mode)
            .anisotropy_enable(anisotropy_enable)
            .max_anisotropy(unsafe {
                device
                    .instance
                    .get_physical_device_properties(device.physical_device)
                    .limits
                    .max_sampler_anisotropy
            })
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .mip_lod_bias(0.0)
            .min_lod(0.0);

        if let Some((mipmap_mode, mip_levels)) = mipmap_info {
            sampler_info.mipmap_mode = mipmap_mode;
            sampler_info.max_lod = mip_levels as f32;
        }

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
