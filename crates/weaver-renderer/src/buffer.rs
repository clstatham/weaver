use std::{num::NonZeroU64, sync::Arc};

use encase::{
    ShaderSize, ShaderType,
    internal::{WriteInto, Writer},
};

use crate::GpuResourceId;

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

/// A GPU buffer. Clones share the buffer and its [`GpuResourceId`].
#[derive(Clone)]
pub struct GpuBuffer {
    gpu_buffer: Arc<wgpu::Buffer>,
    id: GpuResourceId,
}

impl GpuBuffer {
    pub fn clone_inner(&self) -> Arc<wgpu::Buffer> {
        self.gpu_buffer.clone()
    }

    pub fn id(&self) -> GpuResourceId {
        self.id
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
            id: GpuResourceId::new(),
        }
    }
}

impl std::ops::Deref for GpuBuffer {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.gpu_buffer
    }
}

/// A growable array of `T` mirrored into a GPU buffer.
///
/// Values are written into a CPU-side staging vec, and [`GpuBufferVec::enqueue_update`] uploads
/// them, reallocating the GPU buffer (with a new [`GpuResourceId`]) if it's too small. Bind
/// groups built through [`crate::bind_group::BindingsBuilder::buffer_vec`] bind exactly the
/// uploaded elements and notice reallocations and length changes on their own.
pub struct GpuBufferVec<T: ShaderType + WriteInto> {
    buffer: Option<GpuBuffer>,
    data: Vec<u8>,
    uploaded_len: usize,
    capacity: usize,
    usage: wgpu::BufferUsages,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ShaderType + WriteInto> GpuBufferVec<T> {
    pub fn new(usage: wgpu::BufferUsages) -> Self {
        Self {
            buffer: None,
            data: Vec::new(),
            uploaded_len: 0,
            capacity: 0,
            usage,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn buffer(&self) -> Option<&GpuBuffer> {
        self.buffer.as_ref()
    }

    /// Binds the uploaded elements (not the whole, possibly larger, buffer), so `arrayLength` in
    /// a shader is the element count. `None` if nothing has been uploaded yet.
    pub fn binding(&'_ self) -> Option<wgpu::BindingResource<'_>> {
        let buffer = self.buffer()?;
        let size = NonZeroU64::new(self.uploaded_len as u64 * u64::from(T::min_size()))?;
        Some(wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: 0,
            size: Some(size),
        }))
    }

    /// The number of elements in the GPU buffer as of the last upload.
    pub fn uploaded_len(&self) -> usize {
        self.uploaded_len
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

    pub fn reserve(&mut self, capacity: usize, device: &wgpu::Device) -> bool {
        if capacity <= self.capacity {
            return false;
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
        true
    }

    /// Uploads the staged elements, growing the GPU buffer if needed (which gives it a new
    /// [`GpuResourceId`]). Uploading zero elements leaves the GPU buffer alone but sets
    /// [`GpuBufferVec::uploaded_len`] to 0, so [`GpuBufferVec::binding`] returns `None`.
    pub fn enqueue_update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.uploaded_len = self.len();
        if self.data.is_empty() {
            return;
        }

        self.reserve(self.uploaded_len, device);
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

    pub fn capacity(&self) -> usize {
        self.storage_buffer.capacity()
    }

    pub fn reserve(&mut self, capacity: usize, device: &wgpu::Device) {
        self.storage_buffer.reserve(capacity, device);
    }

    pub fn push(&mut self, value: T) -> GpuArrayBufferIndex<T> {
        let index = self.storage_buffer.push(value) as u32;
        GpuArrayBufferIndex {
            index,
            _phantom: std::marker::PhantomData,
        }
    }

    /// See [`GpuBufferVec::enqueue_update`].
    pub fn enqueue_update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.storage_buffer.enqueue_update(device, queue)
    }

    pub fn buffer_vec(&self) -> &GpuBufferVec<T> {
        &self.storage_buffer
    }

    pub fn clear(&mut self) {
        self.storage_buffer.clear();
    }

    pub fn binding(&'_ self) -> Option<wgpu::BindingResource<'_>> {
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
