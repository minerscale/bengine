use std::{iter::zip, mem::offset_of, ops::Deref, ptr::slice_from_raw_parts, rc::Rc};

use ash::{khr, vk};
use log::info;

use crate::{instance::Instance, surface::Surface};

pub struct Device {
    pub device: Rc<ash::Device>,

    pub physical_device: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub graphics_index: u32,
    pub present_index: u32,
    pub mssa_samples: vk::SampleCountFlags,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
}

fn pick_physical_device(
    instance: &ash::Instance,
    surface: &Surface,
    physical_devices: Vec<vk::PhysicalDevice>,
    requested_features: &vk::PhysicalDeviceFeatures,
    requested_features12: &vk::PhysicalDeviceVulkan12Features,
    //requested_features13: &vk::PhysicalDeviceVulkan13Features,
) -> Option<(vk::PhysicalDevice, (u32, u32), vk::SampleCountFlags)> {
    fn feature_subset(
        requested_features: &vk::PhysicalDeviceFeatures,
        capabilities: &vk::PhysicalDeviceFeatures,
    ) -> bool {
        let features_len = size_of::<vk::PhysicalDeviceFeatures>() / size_of::<vk::Bool32>();

        // safety: vk::PhysicalDeviceFeatures is a repr(C) struct containing only VkBool32s,
        //         effectively making it an array and we can cast it accordingly.
        let features_to_slice = |features: &vk::PhysicalDeviceFeatures| unsafe {
            slice_from_raw_parts(
                &features.robust_buffer_access as *const vk::Bool32,
                features_len,
            )
            .as_ref()
            .unwrap()
        };

        !zip(
            features_to_slice(requested_features),
            features_to_slice(capabilities),
        )
        .any(|(&requested, &capability)| requested != 0 && capability == 0)
    }

    fn feature_subset12(
        requested_features: &vk::PhysicalDeviceVulkan12Features,
        capabilities: &vk::PhysicalDeviceVulkan12Features,
    ) -> bool {
        let features_len = (size_of::<vk::PhysicalDeviceVulkan12Features>()
            - offset_of!(
                vk::PhysicalDeviceVulkan12Features,
                sampler_mirror_clamp_to_edge
            ))
            / size_of::<vk::Bool32>();

        // safety: vk::PhysicalDeviceFeatures is a repr(C) struct containing only VkBool32s,
        //         effectively making it an array and we can cast it accordingly.
        let features_to_slice = |features: &vk::PhysicalDeviceVulkan12Features| unsafe {
            slice_from_raw_parts(
                &features.sampler_mirror_clamp_to_edge as *const vk::Bool32,
                features_len,
            )
            .as_ref()
            .unwrap()
        };

        !zip(
            features_to_slice(requested_features),
            features_to_slice(capabilities),
        )
        .any(|(&requested, &capability)| requested != 0 && capability == 0)
    }

    /*fn feature_subset13(
        requested_features: &vk::PhysicalDeviceVulkan13Features,
        capabilities: &vk::PhysicalDeviceVulkan13Features,
    ) -> bool {
        let features_len = (size_of::<vk::PhysicalDeviceVulkan13Features>()
            - offset_of!(vk::PhysicalDeviceVulkan13Features, robust_image_access))
            / size_of::<vk::Bool32>();

        // safety: vk::PhysicalDeviceFeatures is a repr(C) struct containing only VkBool32s,
        //         effectively making it an array and we can cast it accordingly.
        let features_to_slice = |features: &vk::PhysicalDeviceVulkan13Features| unsafe {
            slice_from_raw_parts(
                &features.robust_image_access as *const vk::Bool32,
                features_len,
            )
            .as_ref()
            .unwrap()
        };

        !zip(
            features_to_slice(requested_features),
            features_to_slice(capabilities),
        )
        .any(|(&requested, &capability)| requested != 0 && capability == 0)
    }*/

    physical_devices.iter().find_map(|physical_device| unsafe {
        /*let mut features13 = vk::PhysicalDeviceVulkan13Features::default();*/
        let mut features12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut features = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut features12)
            /*.push_next(&mut features13)*/;

        instance.get_physical_device_features2(*physical_device, &mut features);

        if !(feature_subset(requested_features, &features.features)
            && feature_subset12(requested_features12, &features12)
            /*&& feature_subset13(requested_features13, &features13)*/)
        {
            return None;
        }

        let physical_device_properties = instance.get_physical_device_properties(*physical_device);

        let sample_count = physical_device_properties
            .limits
            .framebuffer_color_sample_counts
            & physical_device_properties
                .limits
                .framebuffer_depth_sample_counts;

        let max_usable_sample_count = 'label: {
            /*if sample_count.contains(vk::SampleCountFlags::TYPE_64) {
                break 'label vk::SampleCountFlags::TYPE_64;
            }
            if sample_count.contains(vk::SampleCountFlags::TYPE_32) {
                break 'label vk::SampleCountFlags::TYPE_32;
            }
            if sample_count.contains(vk::SampleCountFlags::TYPE_16) {
                break 'label vk::SampleCountFlags::TYPE_16;
            }*/
            if sample_count.contains(vk::SampleCountFlags::TYPE_8) {
                break 'label vk::SampleCountFlags::TYPE_8;
            }
            if sample_count.contains(vk::SampleCountFlags::TYPE_4) {
                break 'label vk::SampleCountFlags::TYPE_4;
            }
            if sample_count.contains(vk::SampleCountFlags::TYPE_2) {
                break 'label vk::SampleCountFlags::TYPE_2;
            }

            vk::SampleCountFlags::TYPE_1
        };

        info!("Multisampling level: {max_usable_sample_count:?}");

        let mut graphics_index = Option::<u32>::None;
        let mut present_index = Option::<u32>::None;

        instance
            .get_physical_device_queue_family_properties(*physical_device)
            .iter()
            .enumerate()
            .find_map(|(index, info)| {
                if graphics_index.is_none() && info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    graphics_index = Some(index as u32);
                }

                if present_index.is_none()
                    && surface
                        .loader
                        .get_physical_device_surface_support(
                            *physical_device,
                            index as u32,
                            **surface,
                        )
                        .unwrap()
                {
                    present_index = Some(index as u32);
                }

                if let (Some(graphics_index), Some(present_index)) = (graphics_index, present_index)
                {
                    Some((
                        *physical_device,
                        (graphics_index, present_index),
                        max_usable_sample_count,
                    ))
                } else {
                    None
                }
            })
    })
}

impl Device {
    pub fn new(instance: &Instance, surface: &Surface) -> Self {
        let features = vk::PhysicalDeviceFeatures::default().sampler_anisotropy(true);
        let mut features12 = vk::PhysicalDeviceVulkan12Features::default();
        /*let mut features13 = vk::PhysicalDeviceVulkan13Features::default()*/;

        let physical_devices = unsafe { instance.enumerate_physical_devices() }.unwrap();
        let (physical_device, (graphics_index, present_index), mssa_samples) =
            pick_physical_device(
                instance,
                surface,
                physical_devices,
                &features,
                &features12,
                //&features13,
            )
            .expect("Couldn't find suitable device");

        let device_memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let device_extension_names = [khr::swapchain::NAME.as_ptr()];

        let priorities = [1.0];

        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(graphics_index)
            .queue_priorities(&priorities);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&device_extension_names)
            .enabled_features(&features)
            .push_next(&mut features12)
            /*.push_next(&mut features13)*/;

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
            mssa_samples,
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
