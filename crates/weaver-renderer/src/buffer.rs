//! GPU buffers and CPU-mirrored arrays of shader types.

use std::{num::NonZeroU64, sync::Arc};

use encase::{
    ShaderSize, ShaderType,
    internal::{WriteInto, Writer},
};

use crate::GpuResourceId;

/// Where in a buffer to write.
pub enum BufferOffset {
    /// The start of the buffer.
    Zero,
    /// An offset in bytes.
    ByteOffset(u64),
    /// An element index, scaled by the element's size when written.
    Index(u64),
}

impl BufferOffset {
    /// The byte offset, for elements of type `T`.
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
    /// A shared handle to the wgpu buffer, without the id.
    pub fn clone_inner(&self) -> Arc<wgpu::Buffer> {
        self.gpu_buffer.clone()
    }

    /// Identifies this allocation; bind groups compare it to detect a replaced buffer.
    pub fn id(&self) -> GpuResourceId {
        self.id
    }

    /// Queues a write of one value at `offset`, serialized with encase.
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
    /// Creates an empty vec. `COPY_DST` is added to `usage` when the GPU buffer is allocated.
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

    /// The GPU buffer, if one has been allocated.
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

    /// Number of elements the GPU buffer has room for.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of elements staged on the CPU side.
    pub fn len(&self) -> usize {
        self.data.len() / u64::from(T::min_size()) as usize
    }

    /// True if no elements are staged.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Stages a value and returns its index. Nothing reaches the GPU until
    /// [`GpuBufferVec::enqueue_update`].
    pub fn push(&mut self, value: T) -> usize {
        let size = u64::from(T::min_size()) as usize;
        let offset = self.data.len();

        self.data.resize(offset + size, 0);

        let mut dest = &mut self.data[offset..(offset + size)];
        value.write_into(&mut Writer::new(&value, &mut dest, 0).unwrap());

        offset / size
    }

    /// Overwrites the staged element at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn replace(&mut self, index: usize, value: T) {
        let size = u64::from(T::min_size()) as usize;
        let offset = index * size;

        assert!(offset + size <= self.data.len(), "index out of bounds");

        let mut dest = &mut self.data[offset..(offset + size)];
        value.write_into(&mut Writer::new(&value, &mut dest, 0).unwrap());
    }

    /// Makes sure the GPU buffer has room for `capacity` elements, allocating a new one (with a
    /// new [`GpuResourceId`]) if not. Returns true if it reallocated. The old contents are not
    /// copied over; [`GpuBufferVec::enqueue_update`] writes everything again.
    pub fn reserve(&mut self, capacity: usize, device: &wgpu::Device) -> bool {
        if capacity <= self.capacity {
            return false;
        }

        // TODO: grows to the exact length with no doubling, so every growth reallocates and
        // rebuilds bind groups
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

    /// Drops all staged elements. The GPU buffer and its last upload are left as they were.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Keeps only the first `len` staged elements.
    pub fn truncate(&mut self, len: usize) {
        self.data.truncate(len * u64::from(T::min_size()) as usize);
    }
}

/// Types that can go in a [`GpuArrayBuffer`]. Implemented for everything that qualifies.
pub trait GpuArrayBufferable: ShaderType + WriteInto + ShaderSize + Clone {}
impl<T: ShaderType + WriteInto + ShaderSize + Clone> GpuArrayBufferable for T {}

/// A [`GpuBufferVec`] with `STORAGE` usage that hands out typed indices when pushing.
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
    /// Creates an empty array buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// The GPU buffer, if allocated.
    pub fn buffer(&self) -> Option<&GpuBuffer> {
        self.storage_buffer.buffer()
    }

    /// Number of elements the GPU buffer has room for.
    pub fn capacity(&self) -> usize {
        self.storage_buffer.capacity()
    }

    /// See [`GpuBufferVec::reserve`].
    pub fn reserve(&mut self, capacity: usize, device: &wgpu::Device) {
        self.storage_buffer.reserve(capacity, device);
    }

    /// Stages a value and returns its index.
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

    /// The underlying vec.
    pub fn buffer_vec(&self) -> &GpuBufferVec<T> {
        &self.storage_buffer
    }

    /// Drops all staged elements.
    pub fn clear(&mut self) {
        self.storage_buffer.clear();
    }

    /// See [`GpuBufferVec::binding`].
    pub fn binding(&'_ self) -> Option<wgpu::BindingResource<'_>> {
        self.storage_buffer.binding()
    }

    /// The layout binding type: a read-only storage buffer of `T`s.
    pub fn binding_type() -> wgpu::BindingType {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: Some(T::min_size()),
        }
    }
}

/// The index of an element pushed to a [`GpuArrayBuffer`].
pub struct GpuArrayBufferIndex<T: GpuArrayBufferable> {
    index: u32,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: GpuArrayBufferable> GpuArrayBufferIndex<T> {
    /// The index as a number.
    pub fn index(&self) -> u32 {
        self.index
    }
}
