use std::{iter::zip, mem::offset_of, ops::Deref, ptr::slice_from_raw_parts};

use ash::{khr, vk};
use easy_cast::Cast;
use log::{debug, info, warn};

use crate::renderer::{
    debug_messenger::{DebugMessenger, ENABLE_VALIDATION_LAYERS},
    instance::{Instance, TARGET_API_VERSION},
    surface::Surface,
};

pub struct Device {
    pub physical_device: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub graphics_index: u32,
    pub present_index: u32,
    pub msaa_samples: vk::SampleCountFlags,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub surface: Surface,
    pub debug_callback: Option<DebugMessenger>,
    pub device: ash::Device,
    pub instance: Instance,
    pub entry: ash::Entry,
}

macro_rules! feature_subset {
    ($requested_features:expr, $capabilities:expr, $t:ty, $first:ident, $last:ident) => {{
        // safety: vk::PhysicalDeviceFeatures and co are a repr(C) struct containing only VkBool32s,
        //         effectively making it an array and we can cast it accordingly.
        let features_to_slice = |features: &$t| {
            slice_from_raw_parts(
                &raw const features.$first,
                ((offset_of!($t, $last) - offset_of!($t, $first)) / size_of::<vk::Bool32>()) + 1,
            )
            .as_ref()
            .unwrap()
        };

        !zip(
            features_to_slice($requested_features),
            features_to_slice($capabilities),
        )
        .any(|(&requested, &capability)| requested != 0 && capability == 0)
    }};
}

fn pick_physical_device(
    instance: &ash::Instance,
    surface: &Surface,
    physical_devices: &[vk::PhysicalDevice],
    requested_features: &vk::PhysicalDeviceFeatures,
    requested_features11: &vk::PhysicalDeviceVulkan11Features,
    requested_features12: &vk::PhysicalDeviceVulkan12Features,
    requested_features13: &vk::PhysicalDeviceVulkan13Features,
) -> Option<(vk::PhysicalDevice, (u32, u32), vk::SampleCountFlags)> {
    physical_devices.iter().find_map(|physical_device| unsafe {
        let mut features11 = vk::PhysicalDeviceVulkan11Features::default();
        let mut features12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut features13 = vk::PhysicalDeviceVulkan13Features::default();

        let features = vk::PhysicalDeviceFeatures2::default();

        let mut features = if TARGET_API_VERSION >= vk::API_VERSION_1_1 {
            if TARGET_API_VERSION >= vk::API_VERSION_1_2 {
                if TARGET_API_VERSION >= vk::API_VERSION_1_3 {
                    features.push_next(&mut features13)
                } else {
                    features
                }
                .push_next(&mut features12)
            } else {
                features
            }
            .push_next(&mut features11)
        } else {
            features
        };

        instance.get_physical_device_features2(*physical_device, &mut features);

        if !feature_subset!(
            requested_features,
            &features.features,
            vk::PhysicalDeviceFeatures,
            robust_buffer_access,
            inherited_queries
        ) || ((TARGET_API_VERSION >= vk::API_VERSION_1_1)
            && !feature_subset!(
                requested_features11,
                &features11,
                vk::PhysicalDeviceVulkan11Features,
                storage_buffer16_bit_access,
                shader_draw_parameters
            ))
            || ((TARGET_API_VERSION >= vk::API_VERSION_1_2)
                && !feature_subset!(
                    requested_features12,
                    &features12,
                    vk::PhysicalDeviceVulkan12Features,
                    sampler_mirror_clamp_to_edge,
                    subgroup_broadcast_dynamic_id
                ))
            || ((TARGET_API_VERSION >= vk::API_VERSION_1_3)
                && !feature_subset!(
                    requested_features13,
                    &features13,
                    vk::PhysicalDeviceVulkan13Features,
                    robust_image_access,
                    maintenance4
                ))
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
            if sample_count.contains(vk::SampleCountFlags::TYPE_64) {
                break 'label vk::SampleCountFlags::TYPE_64;
            }
            if sample_count.contains(vk::SampleCountFlags::TYPE_32) {
                break 'label vk::SampleCountFlags::TYPE_32;
            }
            if sample_count.contains(vk::SampleCountFlags::TYPE_16) {
                break 'label vk::SampleCountFlags::TYPE_16;
            }
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

        let chosen_sample_count = max_usable_sample_count
            .clamp(vk::SampleCountFlags::TYPE_1, vk::SampleCountFlags::TYPE_8);

        info!("Multisampling level: {chosen_sample_count:?}");

        let mut graphics_index = Option::<u32>::None;
        let mut present_index = Option::<u32>::None;

        instance
            .get_physical_device_queue_family_properties(*physical_device)
            .iter()
            .enumerate()
            .find_map(|(index, info)| {
                let index: u32 = index.cast();

                if graphics_index.is_none() && info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                    graphics_index = Some(index);
                }

                if present_index.is_none()
                    && surface
                        .loader
                        .get_physical_device_surface_support(*physical_device, index, **surface)
                        .unwrap()
                {
                    present_index = Some(index);
                }

                if let (Some(graphics_index), Some(present_index)) = (graphics_index, present_index)
                {
                    physical_device_properties
                        .device_name_as_c_str()
                        .ok()
                        .and_then(|name| name.to_str().ok())
                        .map_or_else(
                            || warn!("GPU name is not UTF-8"),
                            |name| info!("GPU: {name}"),
                        );

                    Some((
                        *physical_device,
                        (graphics_index, present_index),
                        chosen_sample_count,
                    ))
                } else {
                    None
                }
            })
    })
}

impl Device {
    pub fn new(window: &sdl3::video::Window) -> Self {
        let entry = ash::Entry::linked();

        let instance = Instance::new(&entry, window);

        let debug_callback = if ENABLE_VALIDATION_LAYERS {
            Some(DebugMessenger::new(&entry, &instance))
        } else {
            None
        };

        let surface = Surface::new(&entry, window, &instance);

        let features = vk::PhysicalDeviceFeatures::default().sampler_anisotropy(true);
        let mut features11 = vk::PhysicalDeviceVulkan11Features::default();
        let mut features12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut features13 = vk::PhysicalDeviceVulkan13Features::default();

        let physical_devices = unsafe { instance.enumerate_physical_devices() }.unwrap();
        let (physical_device, (graphics_index, present_index), msaa_samples) =
            pick_physical_device(
                &instance,
                &surface,
                &physical_devices,
                &features,
                &features11,
                &features12,
                &features13,
            )
            .expect("Couldn't find suitable device");

        let device_memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let mut device_extension_names = [khr::swapchain::NAME.as_ptr()].to_vec();

        let extension_properties = unsafe {
            instance
                .enumerate_device_extension_properties(physical_device)
                .unwrap()
        };

        if extension_properties
            .iter()
            .any(|&s| s.extension_name_as_c_str().unwrap() == khr::portability_subset::NAME)
        {
            device_extension_names.push(khr::portability_subset::NAME.as_ptr());
        }

        let priorities = [1.0];

        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(graphics_index)
            .queue_priorities(&priorities);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&device_extension_names)
            .enabled_features(&features);

        let device_create_info = if TARGET_API_VERSION >= vk::API_VERSION_1_1 {
            if TARGET_API_VERSION >= vk::API_VERSION_1_2 {
                if TARGET_API_VERSION >= vk::API_VERSION_1_3 {
                    device_create_info.push_next(&mut features13)
                } else {
                    device_create_info
                }
                .push_next(&mut features12)
            } else {
                device_create_info
            }
            .push_next(&mut features11)
        } else {
            device_create_info
        };

        let device =
            unsafe { instance.create_device(physical_device, &device_create_info, None) }.unwrap();

        let graphics_queue = unsafe { device.get_device_queue(graphics_index, 0) };
        let present_queue = unsafe { device.get_device_queue(present_index, 0) };

        Self {
            physical_device,
            device_memory_properties,
            graphics_index,
            present_index,
            msaa_samples,
            graphics_queue,
            present_queue,
            surface,
            debug_callback,
            device,
            instance,
            entry,
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
        debug!("dropped device");
        unsafe {
            self.device.destroy_device(None);
        };
    }
}
