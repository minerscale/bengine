use std::{ops::Deref, rc::Rc};

use ash::vk;
use log::info;

use crate::device::Device;

pub struct CommandBuffer {
    device: Rc<ash::Device>,
    command_buffer: vk::CommandBuffer,
    recording: bool,
}

impl CommandBuffer {
    pub fn begin(&mut self, flags: vk::CommandBufferUsageFlags) {
        assert!(self.recording == false);

        self.recording = true;
        unsafe {
            // No need to reset a buffer if we've definitely never used it
            if !flags.contains(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT) {
                self.device
                    .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                    .unwrap();
            }

            let begin_info = vk::CommandBufferBeginInfo::default().flags(flags);
            self.device
                .begin_command_buffer(self.command_buffer, &begin_info)
                .unwrap();
        }
    }

    pub fn end(&mut self) {
        assert!(self.recording == true);

        self.recording = false;

        unsafe {
            self.device
                .end_command_buffer(self.command_buffer)
                .expect("failed to record command buffer")
        };
    }
}

impl Deref for CommandBuffer {
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.command_buffer
    }
}

pub struct CommandPool {
    command_pool: vk::CommandPool,
    device: Rc<ash::Device>,
}

impl CommandPool {
    pub fn create_command_buffer(&self) -> CommandBuffer {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffer =
            unsafe { self.device.allocate_command_buffers(&alloc_info) }.unwrap()[0];

        CommandBuffer {
            device: self.device.clone(),
            command_buffer,
            recording: false,
        }
    }

    pub fn new(device: &Device) -> Self {
        let pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(device.queue_family_index);

        Self {
            device: device.device.clone(),
            command_pool: unsafe { device.create_command_pool(&pool_create_info, None).unwrap() },
        }
    }
}

impl Deref for CommandPool {
    type Target = vk::CommandPool;

    fn deref(&self) -> &Self::Target {
        &self.command_pool
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        info!("dropped command pool");
        unsafe {
            self.device.destroy_command_pool(self.command_pool, None);
        }
    }
}
