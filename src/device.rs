use std::{ops::Deref, rc::Rc};

use ash::{khr, vk};
use log::info;

pub struct Device {
    pub device: Rc<ash::Device>,

    pub physical_device: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub graphics_index: u32,
    pub present_index: u32,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
}

impl Device {
    fn pick_physical_device(
        instance: &ash::Instance,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
        physical_devices: Vec<vk::PhysicalDevice>,
    ) -> Option<(vk::PhysicalDevice, (u32, u32))> {
        physical_devices.iter().find_map(|physical_device| unsafe {
            /*
            let mut features13 = vk::PhysicalDeviceVulkan13Features::default();
            let mut features12 = vk::PhysicalDeviceVulkan12Features::default();
            let mut features = vk::PhysicalDeviceFeatures2::default()
                .push_next(&mut features12)
                .push_next(&mut features13);

            instance.get_physical_device_features2(*physical_device, &mut features);

            if features12.descriptor_indexing == 0
                || features12.buffer_device_address == 0
                || features13.dynamic_rendering == 0
                || features13.synchronization2 == 0
            {
                return None;
            }*/

            let mut graphics_index = Option::<u32>::None;
            let mut present_index = Option::<u32>::None;

            instance
                .get_physical_device_queue_family_properties(*physical_device)
                .iter()
                .enumerate()
                .find_map(|(index, info)| {
                    if graphics_index.is_none()
                        && info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    {
                        graphics_index = Some(index as u32);
                    }

                    if present_index.is_none()
                        && surface_loader
                            .get_physical_device_surface_support(
                                *physical_device,
                                index as u32,
                                surface,
                            )
                            .unwrap()
                    {
                        present_index = Some(index as u32);
                    }

                    if let (Some(graphics_index), Some(present_index)) =
                        (graphics_index, present_index)
                    {
                        Some((*physical_device, (graphics_index, present_index)))
                    } else {
                        None
                    }
                })
        })
    }

    pub fn new(
        instance: &ash::Instance,
        surface_loader: &khr::surface::Instance,
        surface: vk::SurfaceKHR,
    ) -> Self {
        let physical_devices = unsafe { instance.enumerate_physical_devices() }.unwrap();
        let (physical_device, (graphics_index, present_index)) =
            Self::pick_physical_device(instance, surface_loader, surface, physical_devices)
                .expect("Couldn't find suitable device");

        let device_memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let device_extension_names = [khr::swapchain::NAME.as_ptr()];

        let mut features12 = vk::PhysicalDeviceVulkan12Features::default();
        //    .descriptor_indexing(true)
        //    .buffer_device_address(true);

        let mut features13 = vk::PhysicalDeviceVulkan13Features::default();
        //    .dynamic_rendering(true)
        //    .synchronization2(true);

        let features = vk::PhysicalDeviceFeatures::default()
            //    .fill_mode_non_solid(true)
            .shader_clip_distance(true);

        let priorities = [1.0];

        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(graphics_index)
            .queue_priorities(&priorities);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&device_extension_names)
            .enabled_features(&features)
            .push_next(&mut features12)
            .push_next(&mut features13);

        let device = Rc::new(
            unsafe { instance.create_device(physical_device, &device_create_info, None) }.unwrap(),
        );

        let graphics_queue = unsafe { device.get_device_queue(graphics_index, 0) };
        let present_queue = unsafe { device.get_device_queue(present_index, 0) };

        Self {
            device,
            physical_device,
            device_memory_properties,
            graphics_index,
            present_index,
            graphics_queue,
            present_queue,
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
