use std::{marker::PhantomData, ops::Deref, rc::Rc};

use ash::vk;
use log::info;

use crate::command_buffer::CommandPool;

pub struct Buffer<T: Copy> {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    device: Rc<ash::Device>,
    size: vk::DeviceSize,
    phantom: PhantomData<T>,
}

impl<T: Copy> Buffer<T> {
    pub fn copy_buffer(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        queue: vk::Queue,
        command_pool: &CommandPool,
        src: &Self,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> Self {
        let command_buffer = command_pool.create_command_buffer();

        command_buffer.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        let device = &src.device;

        let (buffer, memory) = Self::create_buffer(
            &device,
            instance,
            physical_device,
            src.size,
            usage,
            properties,
        );

        let copy_region = [vk::BufferCopy {
            src_offset: 0,
            dst_offset: 0,
            size: src.size,
        }];

        unsafe { device.cmd_copy_buffer(*command_buffer, **src, buffer, &copy_region) };

        command_buffer.end();

        let command_buffer_list = [*command_buffer];

        let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffer_list);
        unsafe {
            device
                .queue_submit(queue, &[submit_info], vk::Fence::null())
                .unwrap();
            device.queue_wait_idle(queue).unwrap();
        };

        unsafe { device.free_command_buffers(**command_pool, &command_buffer_list) };

        Self {
            buffer,
            memory,
            device: src.device.clone(),
            size: src.size,
            phantom: PhantomData,
        }
    }

    fn create_buffer(
        device: &ash::Device,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> (vk::Buffer, vk::DeviceMemory) {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None).unwrap() };
        let memory_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(memory_requirements.size)
            .memory_type_index(Self::find_memory_type(
                instance,
                physical_device,
                memory_requirements.memory_type_bits,
                properties,
            ));

        let memory = unsafe { device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { device.bind_buffer_memory(buffer, memory, 0).unwrap() }

        (buffer, memory)
    }

    pub fn new(
        device: Rc<ash::Device>,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        data: &[T],
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> Self {
        let size: vk::DeviceSize = (data.len() * size_of::<T>()).try_into().unwrap();

        let (buffer, memory) =
            Self::create_buffer(&device, instance, physical_device, size, usage, properties);
        {
            let mapped_memory = unsafe {
                std::slice::from_raw_parts_mut(
                    device
                        .map_memory(memory, 0, size, vk::MemoryMapFlags::empty())
                        .unwrap() as *mut T,
                    data.len(),
                )
            };
            mapped_memory.copy_from_slice(data);
            unsafe { device.unmap_memory(memory) };
        }

        Self {
            buffer,
            memory,
            device,
            size,
            phantom: PhantomData,
        }
    }

    fn find_memory_type(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> u32 {
        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        for i in 0..memory_properties.memory_type_count {
            if (type_filter & (1 << i) > 0)
                && (memory_properties.memory_types[i as usize].property_flags & properties
                    == properties)
            {
                return i;
            }
        }

        panic!("failed to find suitable memory type");
    }
}

impl<T: Copy> Deref for Buffer<T> {
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: Copy> Drop for Buffer<T> {
    fn drop(&mut self) {
        info!("dropped Buffer");
        unsafe {
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.memory, None);
        };
    }
}
