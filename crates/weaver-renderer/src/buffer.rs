use std::sync::Arc;

use encase::{
    internal::{WriteInto, Writer},
    ShaderSize, ShaderType,
};

pub enum BufferOffset {
    Zero,
    ByteOffset(u64),
    Index(u64),
}

impl BufferOffset {
    pub fn calculate<T: ShaderType>(self) -> u64 {
        match self {
            BufferOffset::Zero => 0,
            BufferOffset::ByteOffset(offset) => offset,
            BufferOffset::Index(index) => index * u64::from(T::min_size()),
        }
    }
}

#[derive(Clone)]
pub struct GpuBuffer {
    gpu_buffer: Arc<wgpu::Buffer>,
}

impl GpuBuffer {
    pub fn clone_inner(&self) -> Arc<wgpu::Buffer> {
        self.gpu_buffer.clone()
    }

    pub fn enqueue_update<T: ShaderType + WriteInto>(
        &self,
        data: T,
        offset: BufferOffset,
        queue: &wgpu::Queue,
    ) {
        let mut buf = Vec::<u8>::new();
        data.write_into(&mut Writer::new(&data, &mut buf, 0).unwrap());

        let offset = offset.calculate::<T>();

        queue.write_buffer(&self.gpu_buffer, offset, &buf);
    }
}

impl From<wgpu::Buffer> for GpuBuffer {
    fn from(buffer: wgpu::Buffer) -> Self {
        Self {
            gpu_buffer: Arc::new(buffer),
        }
    }
}

impl std::ops::Deref for GpuBuffer {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.gpu_buffer
    }
}

pub struct GpuBufferVec<T: ShaderType + WriteInto> {
    buffer: Option<GpuBuffer>,
    data: Vec<u8>,
    capacity: usize,
    usage: wgpu::BufferUsages,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ShaderType + WriteInto> GpuBufferVec<T> {
    pub fn new(usage: wgpu::BufferUsages) -> Self {
        Self {
            buffer: None,
            data: Vec::new(),
            capacity: 0,
            usage,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn buffer(&self) -> Option<&GpuBuffer> {
        self.buffer.as_ref()
    }

    pub fn binding(&self) -> Option<wgpu::BindingResource> {
        let buffer = self.buffer()?;
        Some(wgpu::BindingResource::Buffer(
            buffer.as_entire_buffer_binding(),
        ))
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.data.len() / u64::from(T::min_size()) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn push(&mut self, value: T) -> usize {
        let size = u64::from(T::min_size()) as usize;
        let offset = self.data.len();

        self.data.resize(offset + size, 0);

        let mut dest = &mut self.data[offset..(offset + size)];
        value.write_into(&mut Writer::new(&value, &mut dest, 0).unwrap());

        offset / size
    }

    pub fn replace(&mut self, index: usize, value: T) {
        let size = u64::from(T::min_size()) as usize;
        let offset = index * size;

        assert!(offset + size <= self.data.len(), "index out of bounds");

        let mut dest = &mut self.data[offset..(offset + size)];
        value.write_into(&mut Writer::new(&value, &mut dest, 0).unwrap());
    }

    pub fn reserve(&mut self, capacity: usize, device: &wgpu::Device) {
        if capacity <= self.capacity {
            return;
        }

        self.capacity = capacity;
        let size = self.capacity * u64::from(T::min_size()) as usize;
        self.buffer = Some(GpuBuffer::from(device.create_buffer(
            &wgpu::BufferDescriptor {
                label: None,
                size: size as u64,
                usage: self.usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        )));
    }

    pub fn enqueue_update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.data.is_empty() {
            return;
        }

        self.reserve(self.data.len() / u64::from(T::min_size()) as usize, device);
        if let Some(buffer) = self.buffer.as_ref() {
            queue.write_buffer(buffer, 0, &self.data);
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn truncate(&mut self, len: usize) {
        self.data.truncate(len * u64::from(T::min_size()) as usize);
    }
}

pub trait GpuArrayBufferable: ShaderType + WriteInto + ShaderSize + Clone {}
impl<T: ShaderType + WriteInto + ShaderSize + Clone> GpuArrayBufferable for T {}

pub struct GpuArrayBuffer<T: GpuArrayBufferable> {
    storage_buffer: GpuBufferVec<T>,
}

impl<T: GpuArrayBufferable> Default for GpuArrayBuffer<T> {
    fn default() -> Self {
        Self {
            storage_buffer: GpuBufferVec::new(wgpu::BufferUsages::STORAGE),
        }
    }
}

impl<T: GpuArrayBufferable> GpuArrayBuffer<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn buffer(&self) -> Option<&GpuBuffer> {
        self.storage_buffer.buffer()
    }

    pub fn push(&mut self, value: T) -> GpuArrayBufferIndex<T> {
        let index = self.storage_buffer.push(value) as u32;
        GpuArrayBufferIndex {
            index,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn euqueue_update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.storage_buffer.enqueue_update(device, queue);
    }

    pub fn clear(&mut self) {
        self.storage_buffer.clear();
    }

    pub fn binding(&self) -> Option<wgpu::BindingResource> {
        self.storage_buffer.binding()
    }

    pub fn binding_type() -> wgpu::BindingType {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: Some(T::min_size()),
        }
    }
}

pub struct GpuArrayBufferIndex<T: GpuArrayBufferable> {
    index: u32,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: GpuArrayBufferable> GpuArrayBufferIndex<T> {
    pub fn index(&self) -> u32 {
        self.index
    }
}
