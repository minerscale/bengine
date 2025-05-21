use std::{ops::Deref, rc::Rc};

use ash::vk;
use log::debug;

pub struct Fence {
    fence: vk::Fence,
    device: Rc<ash::Device>,
}

impl Fence {
    pub fn new(device: Rc<ash::Device>) -> Self {
        let fence_create_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        Self {
            fence: unsafe { device.create_fence(&fence_create_info, None).unwrap() },
            device,
        }
    }
}

impl Deref for Fence {
    type Target = vk::Fence;

    fn deref(&self) -> &Self::Target {
        &self.fence
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        debug!("dropped fence");
        unsafe { self.device.destroy_fence(self.fence, None) };
    }
}

pub struct Semaphore {
    semaphore: vk::Semaphore,
    device: Rc<ash::Device>,
}

impl Semaphore {
    pub fn new(device: Rc<ash::Device>) -> Self {
        let semaphore_create_info = vk::SemaphoreCreateInfo::default();

        Self {
            semaphore: unsafe {
                device
                    .create_semaphore(&semaphore_create_info, None)
                    .unwrap()
            },
            device,
        }
    }
}

impl Deref for Semaphore {
    type Target = vk::Semaphore;

    fn deref(&self) -> &Self::Target {
        &self.semaphore
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        debug!("dropped semaphore");
        unsafe { self.device.destroy_semaphore(self.semaphore, None) };
    }
}
