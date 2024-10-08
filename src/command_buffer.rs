use std::ops::Deref;

use ash::vk;
use log::info;

use crate::{device::Device, pipeline::Pipeline, swapchain::Swapchain};

pub struct CommandPool {
    device: ash::Device,
    command_pool: vk::CommandPool,
    pub command_buffers: Vec<CommandBuffer>,
}

#[derive(Clone)]
pub struct CommandBuffer {
    device: ash::Device,
    command_buffer: vk::CommandBuffer,
}

impl CommandBuffer {
    pub fn record(&self, image_index: u32, pipeline: &Pipeline, swapchain: &Swapchain) {
        unsafe {
            self.device
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap();

            let begin_info = vk::CommandBufferBeginInfo::default();
            self.device
                .begin_command_buffer(self.command_buffer, &begin_info)
                .unwrap();

            let clear_color = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            }];

            let render_pass_info = vk::RenderPassBeginInfo::default()
                .render_pass(*pipeline.render_pass)
                .framebuffer(swapchain.images[image_index as usize].framebuffer.unwrap())
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: swapchain.extent,
                })
                .clear_values(&clear_color);

            let device = &self.device;

            device.cmd_begin_render_pass(
                self.command_buffer,
                &render_pass_info,
                vk::SubpassContents::INLINE,
            );

            device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                **pipeline,
            );

            let viewport = [vk::Viewport::default()
                .x(0.0)
                .y(0.0)
                .width(swapchain.extent.width as f32)
                .height(swapchain.extent.height as f32)
                .min_depth(0.0)
                .max_depth(1.0)];

            device.cmd_set_viewport(self.command_buffer, 0, &viewport);

            let scissor = [vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: swapchain.extent.width,
                    height: swapchain.extent.height,
                },
            }];

            device.cmd_set_scissor(self.command_buffer, 0, &scissor);

            device.cmd_draw(self.command_buffer, 3, 1, 0, 0);

            device.cmd_end_render_pass(self.command_buffer);

            device
                .end_command_buffer(self.command_buffer)
                .expect("failed to record command buffer");
        }
    }

    pub fn submit(
        &self,
        queue: vk::Queue,
        wait_semaphore: &[vk::Semaphore],
        signal_semaphore: &[vk::Semaphore],
        fence: vk::Fence,
    ) {
        let command_buffer = [self.command_buffer];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphore)
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
            .command_buffers(&command_buffer)
            .signal_semaphores(&signal_semaphore);

        unsafe {
            self.device
                .queue_submit(queue, &[submit_info], fence)
                .unwrap()
        }
    }
}

impl Deref for CommandBuffer {
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.command_buffer
    }
}

impl CommandPool {
    pub fn push_command_buffer(&mut self) {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffer =
            unsafe { self.device.allocate_command_buffers(&alloc_info) }.unwrap()[0];

        self.command_buffers.push(CommandBuffer {
            device: self.device.clone(),
            command_buffer,
        })
    }

    pub fn new(device: &Device) -> Self {
        let pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(device.queue_family_index);

        Self {
            device: (*device).clone(),
            command_pool: unsafe { device.create_command_pool(&pool_create_info, None).unwrap() },
            command_buffers: Vec::new(),
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
