use std::ops::Deref;

use ash::{
    khr,
    vk::{self, Handle},
};
use log::debug;

pub struct Surface {
    pub loader: khr::surface::Instance,
    surface: vk::SurfaceKHR,
}

impl Surface {
    pub fn new(entry: &ash::Entry, window: &sdl3::video::Window, instance: &ash::Instance) -> Self {
        let loader = khr::surface::Instance::new(entry, instance);

        let surface = vk::SurfaceKHR::from_raw(
            window
                .vulkan_create_surface(instance.handle().as_raw() as sdl3::video::VkInstance)
                .unwrap() as u64,
        );

        Self { loader, surface }
    }
}

impl Deref for Surface {
    type Target = vk::SurfaceKHR;

    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        debug!("dropped surface");
        unsafe { self.loader.destroy_surface(self.surface, None) };
    }
}
