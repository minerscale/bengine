use std::{
    ffi::{CString, c_char},
    ops::Deref,
};

use ash::{ext, vk};
use log::debug;

use crate::renderer::debug_messenger::ENABLE_VALIDATION_LAYERS;

pub const TARGET_API_VERSION: u32 = vk::API_VERSION_1_2;

pub struct Instance {
    instance: ash::Instance,
}

impl Instance {
    pub fn new(entry: &ash::Entry, window: &sdl3::video::Window) -> Self {
        let app_name = c"Bengine";

        let layer_names: &[&std::ffi::CStr] = if ENABLE_VALIDATION_LAYERS {
            &[c"VK_LAYER_KHRONOS_validation"]
        } else {
            &[]
        };
        let layers_names_raw: Vec<*const c_char> = layer_names
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        let required_instance_extensions = window
            .vulkan_instance_extensions()
            .unwrap()
            .into_iter()
            .map(CString::new)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let mut extension_names = required_instance_extensions
            .iter()
            .map(|s| s.as_ptr())
            .collect::<Vec<_>>();

        if ENABLE_VALIDATION_LAYERS {
            extension_names.push(ext::debug_utils::NAME.as_ptr());
        }

        let instance_create_flags = if cfg!(target_os = "macos") {
            extension_names.push(c"VK_KHR_portability_enumeration".as_ptr());
            vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
        } else {
            vk::InstanceCreateFlags::empty()
        };

        let app_info = vk::ApplicationInfo::default()
            .application_name(app_name)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(c"No Engine")
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(TARGET_API_VERSION);

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&layers_names_raw)
            .enabled_extension_names(&extension_names)
            .flags(instance_create_flags);

        Self {
            instance: unsafe { entry.create_instance(&create_info, None) }.unwrap(),
        }
    }
}

impl Deref for Instance {
    type Target = ash::Instance;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        debug!("dropped instance");
        unsafe { self.instance.destroy_instance(None) };
    }
}
