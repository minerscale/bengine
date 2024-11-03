use std::{mem::MaybeUninit, ops::Deref};

use ash::{
    khr,
    vk::{self, Handle},
};
use log::info;

pub struct Surface {
    pub loader: khr::surface::Instance,
    surface: vk::SurfaceKHR,
}

impl Surface {
    pub fn new(entry: &ash::Entry, window: &sdl2::video::Window, instance: &ash::Instance) -> Self {
        let loader = khr::surface::Instance::new(entry, instance);

        unsafe {
            let mut surface: MaybeUninit<sdl2::sys::VkSurfaceKHR> = MaybeUninit::uninit();

            Surface {
                loader,
                surface: match sdl2::sys::SDL_Vulkan_CreateSurface(
                    window.raw(),
                    instance
                        .handle()
                        .as_raw()
                        .try_into()
                        .map_err(|_| "instance handle should fit in a u32!")
                        .unwrap(),
                    surface.as_mut_ptr(),
                ) {
                    sdl2::sys::SDL_bool::SDL_FALSE => {
                        Err("failed to make vulkan surface".to_owned())
                    }
                    sdl2::sys::SDL_bool::SDL_TRUE => {
                        Ok(vk::SurfaceKHR::from_raw(surface.assume_init()))
                    }
                }
                .unwrap(),
            }
        }
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
        info!("dropped surface");
        unsafe { self.loader.destroy_surface(self.surface, None) };
    }
}
