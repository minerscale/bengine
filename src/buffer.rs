use std::{marker::PhantomData, ops::Deref, rc::Rc};

use ash::vk;
use log::info;

use crate::command_buffer::ActiveCommandBuffer;

pub struct Buffer<T: Copy> {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    device: Rc<ash::Device>,
    size: vk::DeviceSize,
    phantom: PhantomData<T>,
}

pub struct MappedBuffer<T: Copy + 'static> {
    pub buffer: Buffer<T>,
    pub mapped_memory: &'static mut [T],
}

impl<T: Copy + 'static> MappedBuffer<T> {
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
            Buffer::<T>::create_buffer(&device, instance, physical_device, size, usage, properties);

        let mapped_memory = unsafe {
            std::slice::from_raw_parts_mut(
                device
                    .map_memory(memory, 0, size, vk::MemoryMapFlags::empty())
                    .unwrap() as *mut T,
                data.len(),
            )
        };
        mapped_memory.copy_from_slice(data);

        MappedBuffer {
            buffer: Buffer {
                buffer,
                memory,
                device,
                size,
                phantom: PhantomData,
            },
            mapped_memory,
        }
    }
}

impl<T: Copy> std::fmt::Debug for Buffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("buffer", &self.buffer)
            .field("memory", &self.memory)
            .field("size", &self.size)
            .finish()
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

fn copy_buffer<C: ActiveCommandBuffer, T: Copy + 'static>(
    buffer: Rc<Buffer<T>>,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    cmd_buf: &mut C,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> Buffer<T> {
    let device = &buffer.device;

    let (new_buffer, memory) = Buffer::<T>::create_buffer(
        device,
        instance,
        physical_device,
        buffer.size,
        usage,
        properties,
    );

    let copy_region = [vk::BufferCopy {
        src_offset: 0,
        dst_offset: 0,
        size: buffer.size,
    }];

    unsafe { device.cmd_copy_buffer(**cmd_buf, **buffer, new_buffer, &copy_region) };
    cmd_buf.add_dependency(buffer.clone());

    Buffer {
        buffer: new_buffer,
        memory,
        device: buffer.device.clone(),
        size: buffer.size,
        phantom: PhantomData,
    }
}

impl<T: Copy + 'static> Buffer<T> {
    pub fn new_staged<C: ActiveCommandBuffer>(
        instance: &ash::Instance,
        device: Rc<ash::Device>,
        physical_device: vk::PhysicalDevice,
        cmd_buf: &mut C,
        usage: vk::BufferUsageFlags,
        data: &[T],
    ) -> Self {
        let staging_buffer = Rc::new(Buffer::new(
            device,
            instance,
            physical_device,
            data,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        ));

        copy_buffer(
            staging_buffer,
            instance,
            physical_device,
            cmd_buf,
            vk::BufferUsageFlags::TRANSFER_DST | usage,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
    }

    pub fn create_buffer(
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
