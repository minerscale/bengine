use std::{marker::PhantomData, ops::Deref, rc::Rc};

use ash::vk;
use log::info;

use crate::command_buffer::ActiveCommandBuffer;

pub struct StagedBuffer<T: Copy> {
    staging_buffer: Buffer<T>,
    buffer: Buffer<T>,
}

impl<T: Copy> Deref for StagedBuffer<T> {
    type Target = Buffer<T>;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: Copy> StagedBuffer<T> {
    pub fn new(
        instance: &ash::Instance,
        device: Rc<ash::Device>,
        physical_device: vk::PhysicalDevice,
        command_buffer: &dyn ActiveCommandBuffer,
        usage: vk::BufferUsageFlags,
        data: &[T],
    ) -> Self {
        let staging_buffer = Buffer::new(
            device,
            instance,
            physical_device,
            data,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        let buffer = staging_buffer.copy(
            instance,
            physical_device,
            command_buffer,
            vk::BufferUsageFlags::TRANSFER_DST | usage,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );

        Self {
            staging_buffer,
            buffer,
        }
    }

    pub fn upload_new(
        &self,
        data: &[T],
        offset: vk::DeviceSize,
        command_buffer: &dyn ActiveCommandBuffer,
    ) {
        let size: vk::DeviceSize = std::mem::size_of_val(data).try_into().unwrap();

        assert!(size + offset <= self.staging_buffer.size);

        let device = &self.staging_buffer.device;

        let mapped_memory = unsafe {
            std::slice::from_raw_parts_mut(
                device
                    .map_memory(
                        self.staging_buffer.memory,
                        offset,
                        size,
                        vk::MemoryMapFlags::empty(),
                    )
                    .unwrap() as *mut T,
                data.len(),
            )
        };
        mapped_memory.copy_from_slice(data);
        unsafe { device.unmap_memory(self.staging_buffer.memory) };

        let copy_region = [vk::BufferCopy {
            src_offset: offset,
            dst_offset: offset,
            size,
        }];

        unsafe {
            device.cmd_copy_buffer(
                **command_buffer,
                *self.staging_buffer,
                *self.buffer,
                &copy_region,
            )
        };
    }
}

pub struct Buffer<T: Copy> {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    device: Rc<ash::Device>,
    size: vk::DeviceSize,
    phantom: PhantomData<T>,
}

pub fn find_memory_type(
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

impl<T: Copy> Buffer<T> {
    pub fn copy(
        &self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        command_buffer: &dyn ActiveCommandBuffer,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> Self {
        let device = &self.device;

        let (buffer, memory) = Self::create_buffer(
            device,
            instance,
            physical_device,
            self.size,
            usage,
            properties,
        );

        let copy_region = [vk::BufferCopy {
            src_offset: 0,
            dst_offset: 0,
            size: self.size,
        }];

        unsafe { device.cmd_copy_buffer(**command_buffer, **self, buffer, &copy_region) };

        Self {
            buffer,
            memory,
            device: self.device.clone(),
            size: self.size,
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
            .memory_type_index(find_memory_type(
                instance,
                physical_device,
                memory_requirements.memory_type_bits,
                properties,
            ));

        let memory = unsafe { device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { device.bind_buffer_memory(buffer, memory, 0).unwrap() }

        (buffer, memory)
    }

    pub fn len(&self) -> vk::DeviceSize {
        self.size / vk::DeviceSize::try_from(size_of::<T>()).unwrap()
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn new(
        device: Rc<ash::Device>,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        data: &[T],
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> Self {
        let size: vk::DeviceSize = std::mem::size_of_val(data).try_into().unwrap();

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
}

impl<T: Copy> Deref for Buffer<T> {
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: Copy> Drop for Buffer<T> {
    fn drop(&mut self) {
        info!("dropped buffer");
        unsafe {
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.memory, None);
        };
    }
}
