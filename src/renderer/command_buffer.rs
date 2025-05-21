use std::{ops::Deref, rc::Rc};

use ash::vk;
use log::debug;

use crate::renderer::device::Device;

pub trait ActiveCommandBuffer: Deref<Target = vk::CommandBuffer> {
    fn add_dependency(&mut self, dependency: Rc<dyn std::any::Any + 'static>);
}

pub struct OneTimeSubmitCommandBuffer {
    device: Rc<ash::Device>,
    command_buffer: vk::CommandBuffer,
    dependencies: Vec<Rc<dyn std::any::Any>>,
}

impl ActiveCommandBuffer for OneTimeSubmitCommandBuffer {
    fn add_dependency(&mut self, dependency: Rc<dyn std::any::Any>) {
        self.dependencies.push(dependency);
    }
}

impl Deref for OneTimeSubmitCommandBuffer {
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.command_buffer
    }
}

impl OneTimeSubmitCommandBuffer {
    pub fn submit(self, queue: vk::Queue, command_pool: &CommandPool) {
        unsafe {
            self.device
                .end_command_buffer(self.command_buffer)
                .expect("failed to record command buffer");
        }

        let command_buffer_list = [self.command_buffer];

        let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffer_list);
        unsafe {
            self.device
                .queue_submit(queue, &[submit_info], vk::Fence::null())
                .unwrap();

            self.device.queue_wait_idle(queue).unwrap();

            self.device
                .free_command_buffers(**command_pool, &[self.command_buffer]);
        };
    }
}

pub struct MultipleSubmitCommandBuffer {
    device: Rc<ash::Device>,
    command_buffer: vk::CommandBuffer,
}

impl MultipleSubmitCommandBuffer {
    pub fn begin(self) -> ActiveMultipleSubmitCommandBuffer {
        unsafe {
            self.device
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap();

            let begin_info = vk::CommandBufferBeginInfo::default();
            self.device
                .begin_command_buffer(self.command_buffer, &begin_info)
                .unwrap();
        }

        ActiveMultipleSubmitCommandBuffer {
            command_buffer: self,
            dependencies: vec![],
        }
    }

    pub fn submit(
        self,
        queue: vk::Queue,
        dst_stage_mask: vk::PipelineStageFlags,
        wait_semaphore: vk::Semaphore,
        signal_semaphores: vk::Semaphore,
        fence: vk::Fence,
    ) -> Self {
        let command_buffer_list = [self.command_buffer];
        let wait_semaphore_list = [wait_semaphore];
        let signal_semaphore_list = [signal_semaphores];
        let dst_stage_mask_list = [dst_stage_mask];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphore_list)
            .wait_dst_stage_mask(&dst_stage_mask_list)
            .command_buffers(&command_buffer_list)
            .signal_semaphores(&signal_semaphore_list);

        unsafe {
            self.device
                .queue_submit(queue, &[submit_info], fence)
                .unwrap();
        }

        self
    }
}

pub struct ActiveMultipleSubmitCommandBuffer {
    command_buffer: MultipleSubmitCommandBuffer,
    dependencies: Vec<Rc<dyn std::any::Any>>,
}

impl ActiveCommandBuffer for ActiveMultipleSubmitCommandBuffer {
    fn add_dependency(&mut self, dependency: Rc<dyn std::any::Any>) {
        self.dependencies.push(dependency);
    }
}

impl Deref for ActiveMultipleSubmitCommandBuffer {
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.command_buffer.command_buffer
    }
}

impl ActiveMultipleSubmitCommandBuffer {
    pub fn record(self, f: impl FnOnce(Self) -> Self) -> Self {
        f(self)
    }

    pub fn end(self) -> MultipleSubmitCommandBuffer {
        unsafe {
            self.command_buffer
                .device
                .end_command_buffer(self.command_buffer.command_buffer)
                .expect("failed to record command buffer");
        }

        self.command_buffer
    }
}

pub struct CommandPool {
    command_pool: vk::CommandPool,
    device: Rc<ash::Device>,
}

impl CommandPool {
    pub fn one_time_submit<T>(
        &self,
        queue: vk::Queue,
        f: impl FnOnce(&mut OneTimeSubmitCommandBuffer) -> T,
    ) -> T {
        let mut cmd_buf = self.create_one_time_submit_command_buffer();

        let result = f(&mut cmd_buf);

        cmd_buf.submit(queue, self);

        result
    }

    fn create_one_time_submit_command_buffer(&self) -> OneTimeSubmitCommandBuffer {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffer =
            unsafe { self.device.allocate_command_buffers(&alloc_info) }.unwrap()[0];

        unsafe {
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device
                .begin_command_buffer(command_buffer, &begin_info)
                .unwrap();
        }

        OneTimeSubmitCommandBuffer {
            device: self.device.clone(),
            command_buffer,
            dependencies: vec![],
        }
    }

    pub fn create_command_buffer(&self) -> MultipleSubmitCommandBuffer {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffer =
            unsafe { self.device.allocate_command_buffers(&alloc_info) }.unwrap()[0];

        MultipleSubmitCommandBuffer {
            device: self.device.clone(),
            command_buffer,
        }
    }

    pub fn new(device: &Device) -> Self {
        let pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(device.graphics_index);

        Self {
            device: device.device.clone(),
            command_pool: unsafe { device.create_command_pool(&pool_create_info, None).unwrap() },
        }
    }

    #[allow(dead_code)]
    pub fn destroy_command_buffer(&self, command_buffer: MultipleSubmitCommandBuffer) {
        unsafe {
            self.device
                .free_command_buffers(self.command_pool, &[command_buffer.command_buffer]);
        }

        drop(command_buffer);
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
        debug!("dropped command pool");
        unsafe {
            self.device.destroy_command_pool(self.command_pool, None);
        }
    }
}
