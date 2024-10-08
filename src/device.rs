use std::ops::Deref;

use ash::{khr, vk};
use log::info;

pub struct Device {
    device: ash::Device,

    pub physical_device: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,
}

impl Device {
    fn pick_physical_device(
        instance: &ash::Instance,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
        physical_devices: Vec<vk::PhysicalDevice>,
    ) -> Option<(vk::PhysicalDevice, u32)> {
        physical_devices.iter().find_map(|physical_device| unsafe {
            instance
                .get_physical_device_queue_family_properties(*physical_device)
                .iter()
                .enumerate()
                .find_map(|(index, info)| {
                    let supports_graphic_and_surface =
                        info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                            && surface_loader
                                .get_physical_device_surface_support(
                                    *physical_device,
                                    index as u32,
                                    surface,
                                )
                                .unwrap();

                    supports_graphic_and_surface.then_some((*physical_device, index as u32))
                })
        })
    }

    pub fn new(
        instance: &ash::Instance,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> Self {
        let physical_devices = unsafe { instance.enumerate_physical_devices() }.unwrap();
        let (physical_device, queue_family_index) =
            Self::pick_physical_device(&instance, &surface_loader, surface, physical_devices)
                .expect("Couldn't find suitable device");

        let device_memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let device_extension_names = [khr::swapchain::NAME.as_ptr()];

        let features = vk::PhysicalDeviceFeatures {
            shader_clip_distance: 1,
            ..Default::default()
        };
        let priorities = [1.0];

        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&priorities);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&device_extension_names)
            .enabled_features(&features);

        let device =
            unsafe { instance.create_device(physical_device, &device_create_info, None) }.unwrap();

        let presentation_queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        Self {
            device,
            physical_device,
            device_memory_properties,
            queue_family_index,
            present_queue: presentation_queue,
        }
    }
}

impl Deref for Device {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        info!("dropped device");
        unsafe {
            self.device.destroy_device(None);
        };
    }
}
