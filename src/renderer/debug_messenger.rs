use std::{
    borrow::Cow,
    ffi::{CStr, c_void},
};

use ash::{ext, vk};
use colored::Colorize;
use log::info;

pub const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);

pub struct DebugMessenger {
    debug_utils_loader: ext::debug_utils::Instance,
    debug_callback: vk::DebugUtilsMessengerEXT,
}

impl DebugMessenger {
    unsafe extern "system" fn vulkan_debug_callback(
        message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
        message_type: vk::DebugUtilsMessageTypeFlagsEXT,
        p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
        _user_data: *mut c_void,
    ) -> vk::Bool32 {
        let callback_data = unsafe { *p_callback_data };
        let message_id_number = callback_data.message_id_number;

        let message_id_name = if callback_data.p_message_id_name.is_null() {
            Cow::from("")
        } else {
            unsafe { CStr::from_ptr(callback_data.p_message_id_name) }.to_string_lossy()
        };

        let message = if callback_data.p_message_id_name.is_null() {
            Cow::from("")
        } else {
            unsafe { CStr::from_ptr(callback_data.p_message) }.to_string_lossy()
        };

        let msg = format!("{message_type:?} [{message_id_name} ({message_id_number})]: {message}");

        println!(
            "{}\n",
            match message_severity {
                vk::DebugUtilsMessageSeverityFlagsEXT::INFO => msg.white().dimmed(),
                vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => msg.white(),
                vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => msg.yellow(),
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => msg.red(),
                _ => msg.into(),
            }
        );

        vk::FALSE
    }

    pub fn new(entry: &ash::Entry, instance: &ash::Instance) -> Self {
        let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                /*vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                |*/
                vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(Self::vulkan_debug_callback));

        let debug_utils_loader = ext::debug_utils::Instance::new(entry, instance);
        let debug_callback =
            unsafe { debug_utils_loader.create_debug_utils_messenger(&debug_info, None) }.unwrap();

        Self {
            debug_utils_loader,
            debug_callback,
        }
    }
}

impl Drop for DebugMessenger {
    fn drop(&mut self) {
        info!("dropped debug messenger");
        unsafe {
            self.debug_utils_loader
                .destroy_debug_utils_messenger(self.debug_callback, None);
        }
    }
}
