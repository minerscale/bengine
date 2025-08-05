use std::{marker::PhantomData, ops::Deref, sync::Arc};

use ash::vk;
use easy_cast::Cast;
use log::debug;

use crate::renderer::{
    command_buffer::ActiveCommandBuffer,
    descriptors::{DescriptorPool, DescriptorSet, DescriptorSetLayout},
    device::Device,
};

pub struct DeviceMemory {
    memory: vk::DeviceMemory,
    device: Arc<Device>,
}

impl std::fmt::Debug for DeviceMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceMemory")
            .field("memory", &self.memory)
            .finish_non_exhaustive()
    }
}

impl Drop for DeviceMemory {
    fn drop(&mut self) {
        unsafe { self.device.free_memory(self.memory, None) };
    }
}

impl Deref for DeviceMemory {
    type Target = vk::DeviceMemory;

    fn deref(&self) -> &Self::Target {
        &self.memory
    }
}

impl DeviceMemory {
    pub fn new(
        device: Arc<Device>,
        properties: vk::MemoryPropertyFlags,
        memory_requirements: vk::MemoryRequirements,
    ) -> Self {
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(memory_requirements.size)
            .memory_type_index(find_memory_type(
                &device.instance,
                device.physical_device,
                memory_requirements.memory_type_bits,
                properties,
            ));

        let memory = unsafe { device.allocate_memory(&alloc_info, None).unwrap() };

        Self { memory, device }
    }
}

#[derive(Debug)]
pub struct BufferMemory {
    pub memory: Arc<DeviceMemory>,
    pub offset: vk::DeviceSize,
}

impl BufferMemory {
    pub fn new(memory: Arc<DeviceMemory>, offset: vk::DeviceSize) -> Self {
        Self { memory, offset }
    }
}

pub struct Buffer<T: Copy + Sync> {
    pub buffer: vk::Buffer,
    pub memory: Option<BufferMemory>,
    size: vk::DeviceSize,
    device: Arc<Device>,
    phantom: PhantomData<T>,
}

#[allow(dead_code)]
pub struct MappedBuffer<T: Copy + Sync + 'static> {
    pub buffer: Arc<Buffer<T>>,
    pub mapped_memory: &'static mut [T],
    pub descriptor_set: DescriptorSet,
}

impl<T: Copy + Sync + Send + 'static> MappedBuffer<T> {
    pub fn new(
        device: &Arc<Device>,
        data: &[T],
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
        descriptor_pool: &DescriptorPool,
        descriptor_set_layout: &DescriptorSetLayout,
        binding: u32,
    ) -> Self {
        let size: vk::DeviceSize = std::mem::size_of_val(data).cast();

        let (buffer, memory) = Buffer::<T>::create_buffer(device, size, usage, properties);

        let mapped_memory = unsafe {
            std::slice::from_raw_parts_mut(
                device
                    .map_memory(*memory, 0, size, vk::MemoryMapFlags::empty())
                    .unwrap()
                    .cast::<T>(),
                data.len(),
            )
        };
        mapped_memory.copy_from_slice(data);

        let buffer = Arc::new(Buffer {
            buffer,
            memory: Some(BufferMemory::new(Arc::new(memory), 0)),
            size,
            device: device.clone(),
            phantom: PhantomData,
        });

        let mut descriptor_set = descriptor_pool.create_descriptor_set(descriptor_set_layout);

        descriptor_set.bind_buffer(device, binding, buffer.clone());

        Self {
            buffer,
            mapped_memory,
            descriptor_set,
        }
    }
}

impl<T: Copy + Sync> std::fmt::Debug for Buffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("buffer", &self.buffer)
            .field("memory", &self.memory)
            .finish_non_exhaustive()
    }
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

fn copy_buffer<C: ActiveCommandBuffer, T: Copy + Sync + Send + 'static>(
    buffer: Arc<Buffer<T>>,
    cmd_buf: &mut C,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> Arc<Buffer<T>> {
    let device = &buffer.device;

    let size = buffer.size;
    let (new_buffer, memory) = Buffer::<T>::create_buffer(device, size, usage, properties);

    let copy_region = [vk::BufferCopy {
        src_offset: buffer.memory.as_ref().unwrap().offset,
        dst_offset: 0,
        size,
    }];

    unsafe { device.cmd_copy_buffer(**cmd_buf, **buffer, new_buffer, &copy_region) };

    let device = buffer.device.clone();
    cmd_buf.add_dependency(buffer);

    let new_buffer = Arc::new(Buffer {
        buffer: new_buffer,
        memory: Some(BufferMemory::new(Arc::new(memory), 0)),
        size,
        device,
        phantom: PhantomData,
    });

    cmd_buf.add_dependency(new_buffer.clone());

    new_buffer
}

impl<T: Copy + Sync + Send + 'static> Buffer<T> {
    pub fn memory_requirements(&self) -> vk::MemoryRequirements {
        unsafe { self.device.get_buffer_memory_requirements(self.buffer) }
    }

    pub unsafe fn new_uninit(
        device: Arc<Device>,
        usage: vk::BufferUsageFlags,
        num_elements: usize,
    ) -> Self {
        let size = (num_elements * size_of::<T>()).cast();

        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None).unwrap() };

        Self {
            buffer,
            memory: None,
            size,
            device,
            phantom: PhantomData,
        }
    }

    pub unsafe fn bind_memory(&mut self, memory: BufferMemory) {
        unsafe {
            self.device
                .bind_buffer_memory(self.buffer, **memory.memory, memory.offset)
                .unwrap();
        }

        self.memory = Some(memory);
    }

    pub fn new_staged_with<C: ActiveCommandBuffer, F: Fn(&mut [T])>(
        device: &Arc<Device>,
        cmd_buf: &mut C,
        usage: vk::BufferUsageFlags,
        data: F,
        num_elements: usize,
    ) -> Arc<Self> {
        let staging_buffer = Arc::new(Self::new_with(
            device,
            data,
            num_elements,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        ));

        copy_buffer(
            staging_buffer,
            cmd_buf,
            vk::BufferUsageFlags::TRANSFER_DST | usage,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
    }

    pub fn new_staged<C: ActiveCommandBuffer>(
        device: &Arc<Device>,
        cmd_buf: &mut C,
        usage: vk::BufferUsageFlags,
        data: &[T],
    ) -> Arc<Self> {
        Self::new_staged_with(
            device,
            cmd_buf,
            usage,
            |mapped_memory| mapped_memory.copy_from_slice(data),
            data.len(),
        )
    }

    fn create_buffer(
        device: &Arc<Device>,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> (vk::Buffer, DeviceMemory) {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None).unwrap() };

        let memory_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let memory = DeviceMemory::new(device.clone(), properties, memory_requirements);

        unsafe { device.bind_buffer_memory(buffer, *memory, 0).unwrap() }

        (buffer, memory)
    }

    pub fn len(&self) -> vk::DeviceSize {
        self.size / vk::DeviceSize::try_from(size_of::<T>()).unwrap()
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn new_with<F: Fn(&mut [T])>(
        device: &Arc<Device>,
        data: F,
        num_elements: usize,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> Self {
        let size = (num_elements * size_of::<T>()).cast();

        let (buffer, memory) = Self::create_buffer(device, size, usage, properties);
        {
            let mapped_memory = unsafe {
                std::slice::from_raw_parts_mut(
                    device
                        .map_memory(*memory, 0, size, vk::MemoryMapFlags::empty())
                        .unwrap()
                        .cast::<T>(),
                    num_elements,
                )
            };
            data(mapped_memory);

            unsafe { device.unmap_memory(*memory) };
        }

        Self {
            buffer,
            memory: Some(BufferMemory::new(memory.into(), 0)),
            size,
            device: device.clone(),
            phantom: PhantomData,
        }
    }

    pub fn new(
        device: &Arc<Device>,
        data: &[T],
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> Self {
        Self::new_with(
            device,
            |mapped_memory| mapped_memory.copy_from_slice(data),
            data.len(),
            usage,
            properties,
        )
    }
}

impl<T: Copy + Sync> Deref for Buffer<T> {
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: Copy + Sync> Drop for Buffer<T> {
    fn drop(&mut self) {
        debug!("dropped buffer");
        unsafe {
            self.device.destroy_buffer(self.buffer, None);
        };
    }
}
