use std::{fmt::Debug, sync::Arc};

use weaver_util::lock::{Lock, MapRead, Read};

use super::{BindGroupLayoutCache, BindableComponent, GpuResourceManager};

/// The type of a GPU resource.
/// This is used to create the appropriate binding type.
/// This is also used to properly initialize a `LazyGpuHandle`.
#[derive(Clone)]
pub enum GpuResourceType {
    /// A uniform buffer.
    Uniform {
        usage: wgpu::BufferUsages,
        size: usize,
    },
    /// A storage buffer.
    Storage {
        usage: wgpu::BufferUsages,
        size: usize,
        read_only: bool,
    },
    /// A texture.
    Texture {
        width: u32,
        height: u32,
        usage: wgpu::TextureUsages,
        format: wgpu::TextureFormat,
        dimension: wgpu::TextureDimension,
        view_dimension: wgpu::TextureViewDimension,
        depth_or_array_layers: u32,
    },
    /// A texture sampler.
    Sampler {
        address_mode: wgpu::AddressMode,
        filter_mode: wgpu::FilterMode,
        compare: Option<wgpu::CompareFunction>,
    },
}

impl Debug for GpuResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uniform { .. } => write!(f, "Uniform"),
            Self::Storage { .. } => write!(f, "Storage"),
            Self::Texture { .. } => write!(f, "Texture"),
            Self::Sampler { .. } => write!(f, "Sampler"),
        }
    }
}

impl Into<wgpu::BindingType> for &GpuResourceType {
    fn into(self) -> wgpu::BindingType {
        match self {
            GpuResourceType::Uniform { .. } => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            GpuResourceType::Storage { read_only, .. } => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: *read_only,
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            GpuResourceType::Texture { view_dimension, .. } => wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: *view_dimension,
                multisampled: false,
            },
            GpuResourceType::Sampler {
                filter_mode,
                compare,
                ..
            } => {
                let comparison = compare.is_some();
                let filtering = filter_mode != &wgpu::FilterMode::Nearest;
                if comparison {
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)
                } else if filtering {
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                } else {
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering)
                }
            }
        }
    }
}

/// A GPU-allocated resource.
pub enum GpuResource {
    Buffer {
        buffer: wgpu::Buffer,
    },
    Texture {
        texture: wgpu::Texture,
        default_view: wgpu::TextureView,
    },
    Sampler {
        sampler: wgpu::Sampler,
    },
}

impl GpuResource {
    /// Returns the resource as a [`wgpu::BindingResource`] for use in a bind group.
    pub fn as_binding_resource(&self) -> wgpu::BindingResource {
        match self {
            GpuResource::Buffer { buffer } => {
                wgpu::BindingResource::Buffer(buffer.as_entire_buffer_binding())
            }
            GpuResource::Texture { default_view, .. } => {
                wgpu::BindingResource::TextureView(default_view)
            }
            GpuResource::Sampler { sampler } => wgpu::BindingResource::Sampler(sampler),
        }
    }

    /// Returns the resource as a [`wgpu::BindGroupEntry`] for use in a bind group.
    pub fn as_binding(&self, binding: u32) -> wgpu::BindGroupEntry {
        wgpu::BindGroupEntry {
            binding,
            resource: self.as_binding_resource(),
        }
    }

    pub fn as_buffer(&self) -> Option<&wgpu::Buffer> {
        match self {
            GpuResource::Buffer { buffer } => Some(buffer),
            _ => None,
        }
    }

    pub fn as_texture(&self) -> Option<&wgpu::Texture> {
        match self {
            GpuResource::Texture { texture, .. } => Some(texture),
            _ => None,
        }
    }

    pub fn as_sampler(&self) -> Option<&wgpu::Sampler> {
        match self {
            GpuResource::Sampler { sampler } => Some(sampler),
            _ => None,
        }
    }
}

/// The status of a GPU handle (Ready, Pending, or Destroyed).
#[derive(Clone)]
pub enum GpuHandleStatus {
    /// The handle is ready to be used.
    Ready { resource: Arc<GpuResource> },
    /// The handle is pending an update.
    Pending { pending_data: Arc<[u8]> },
    /// The handle has been destroyed.
    Destroyed,
}

impl GpuHandleStatus {
    /// Returns true if the handle is ready to be used.
    pub fn is_ready(&self) -> bool {
        matches!(self, GpuHandleStatus::Ready { .. })
    }

    /// Returns true if the handle is pending an update.
    pub fn is_pending(&self) -> bool {
        matches!(self, GpuHandleStatus::Pending { .. })
    }

    /// Returns true if the handle has been destroyed.
    pub fn is_destroyed(&self) -> bool {
        matches!(self, GpuHandleStatus::Destroyed)
    }
}

impl Debug for GpuHandleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuHandleStatus::Ready { .. } => write!(f, "Ready"),
            GpuHandleStatus::Pending { .. } => write!(f, "Pending"),
            GpuHandleStatus::Destroyed => write!(f, "Destroyed"),
        }
    }
}

/// A handle to a GPU-allocated resource.
#[derive(Clone)]
pub struct GpuHandle {
    pub(super) id: u64,
    pub(super) status: Arc<Lock<GpuHandleStatus>>,
}

impl GpuHandle {
    /// Marks the handle as pending an update.
    /// This will not update the GPU resource until the next frame, unless the render queue is manually flushed.
    pub fn update<T: bytemuck::Pod>(&mut self, data: &[T]) {
        let mut status = self.status.write();
        if !status.is_destroyed() {
            *status = GpuHandleStatus::Pending {
                pending_data: Arc::from(bytemuck::cast_slice(data)),
            };
        } else {
            log::warn!(
                "Attempted to update buffer that is already destroyed: {} is {:?}",
                self.id,
                self.status
            );
        }
    }

    /// Returns the underlying buffer iff the handle is ready and the underlying resource is a buffer.
    pub fn get_buffer(&self) -> Option<MapRead<'_, wgpu::Buffer>> {
        let status = self.status.read();
        if let GpuHandleStatus::Ready {
            resource: ref buffer,
        } = &*status
        {
            match buffer.as_ref() {
                GpuResource::Buffer { .. } => Some(Read::map_read(status, |status| match status {
                    GpuHandleStatus::Ready { resource: buffer } => match buffer.as_ref() {
                        GpuResource::Buffer { buffer } => buffer,
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                })),
                GpuResource::Texture { .. } => {
                    log::warn!(
                        "Attempted to get buffer from texture: {} is {:?}",
                        self.id,
                        self.status
                    );
                    None
                }
                GpuResource::Sampler { .. } => {
                    log::warn!(
                        "Attempted to get buffer from sampler: {} is {:?}",
                        self.id,
                        self.status
                    );
                    None
                }
            }
        } else {
            log::warn!(
                "Attempted to get buffer that is not ready: {} is {:?}",
                self.id,
                self.status
            );
            None
        }
    }

    /// Returns the underlying texture iff the handle is ready and the underlying resource is a texture.
    pub fn get_texture(&self) -> Option<MapRead<'_, wgpu::Texture>> {
        let status = self.status.read();
        if let GpuHandleStatus::Ready {
            resource: ref buffer,
        } = &*status
        {
            match buffer.as_ref() {
                GpuResource::Texture { .. } => {
                    Some(Read::map_read(status, |status| match status {
                        GpuHandleStatus::Ready { resource: buffer } => match buffer.as_ref() {
                            GpuResource::Texture { texture, .. } => texture,
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    }))
                }
                GpuResource::Buffer { .. } => {
                    log::warn!("Attempted to get texture from buffer: {}", self.id,);
                    None
                }
                GpuResource::Sampler { .. } => {
                    log::warn!("Attempted to get texture from sampler: {}", self.id,);
                    None
                }
            }
        } else {
            log::warn!(
                "Attempted to get texture that is not ready: {} is {:?}",
                self.id,
                self.status
            );
            None
        }
    }

    /// Returns the underlying sampler iff the handle is ready and the underlying resource is a sampler.
    pub fn get_sampler(&self) -> Option<MapRead<'_, wgpu::Sampler>> {
        let status = self.status.read();
        if let GpuHandleStatus::Ready {
            resource: ref buffer,
        } = &*status
        {
            match buffer.as_ref() {
                GpuResource::Sampler { .. } => {
                    Some(Read::map_read(status, |status| match status {
                        GpuHandleStatus::Ready { resource: buffer } => match buffer.as_ref() {
                            GpuResource::Sampler { sampler } => sampler,
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    }))
                }
                GpuResource::Buffer { .. } => {
                    log::warn!("Attempted to get sampler from buffer: {}", self.id,);
                    None
                }
                GpuResource::Texture { .. } => {
                    log::warn!("Attempted to get sampler from texture: {}", self.id,);
                    None
                }
            }
        } else {
            log::warn!(
                "Attempted to get sampler that is not ready: {} is {:?}",
                self.id,
                self.status
            );
            None
        }
    }

    /// Marks the underlying GPU resource for destruction, if it is not already destroyed.
    /// This will not destroy the GPU resource until the next frame, unless [`GpuResourceManager::gc_destroyed_resources`] is manually called.
    pub fn mark_destroyed(&mut self) {
        let mut status = self.status.write();
        match &mut *status {
            GpuHandleStatus::Ready { .. } => {
                *status = GpuHandleStatus::Destroyed;
            }
            GpuHandleStatus::Pending { .. } => {
                *status = GpuHandleStatus::Destroyed;
            }
            GpuHandleStatus::Destroyed => {
                log::warn!("Attempted to destroy a buffer that is already destroyed");
            }
        }
    }

    /// Returns true if the handle is ready to be used.
    pub fn is_ready(&self) -> bool {
        self.status.read().is_ready()
    }

    /// Returns true if the handle is pending an update.
    pub fn is_pending(&self) -> bool {
        self.status.read().is_pending()
    }

    /// Returns true if the handle has been destroyed.
    pub fn is_destroyed(&self) -> bool {
        self.status.read().is_destroyed()
    }
}

/// The status of a lazily initialized resource.
#[derive(Clone)]
pub enum LazyInitStatus {
    /// The resource is ready to be used.
    Initialized { handle: GpuHandle },
    /// The resource is pending initialization.
    Uninitialized {
        ty: GpuResourceType,
        label: Option<&'static str>,
        pending_data: Option<Arc<[u8]>>,
    },
    /// The resource has been destroyed.
    Destroyed,
}

impl LazyInitStatus {
    /// Returns true if the resource is initialized and ready to be used.
    pub fn is_initialized(&self) -> bool {
        matches!(self, LazyInitStatus::Initialized { .. })
    }

    /// Returns true if the resource is pending initialization.
    pub fn is_uninitialized(&self) -> bool {
        matches!(self, LazyInitStatus::Uninitialized { .. })
    }

    /// Returns true if the resource has been destroyed.
    pub fn is_destroyed(&self) -> bool {
        matches!(self, LazyInitStatus::Destroyed)
    }
}

impl Debug for LazyInitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LazyInitStatus::Initialized { .. } => write!(f, "Initialized"),
            LazyInitStatus::Uninitialized { ty, .. } => write!(f, "Uninitialized ({:#?})", ty),
            LazyInitStatus::Destroyed => write!(f, "Destroyed"),
        }
    }
}

/// A handle to a GPU resource that is lazily initialized.
/// This is useful for resources that are not used by the GPU until the first frame.
#[derive(Clone, Debug)]
pub struct LazyGpuHandle {
    status: Arc<Lock<LazyInitStatus>>,
    label: Option<&'static str>,
}

impl LazyGpuHandle {
    /// Creates a new `LazyGpuHandle` with the given resource type, and optional label and pending data.
    pub(crate) fn new(
        ty: GpuResourceType,
        label: Option<&'static str>,
        pending_data: Option<Arc<[u8]>>,
    ) -> Self {
        Self {
            status: Arc::new(Lock::new(LazyInitStatus::Uninitialized {
                ty,
                label,
                pending_data,
            })),
            label,
        }
    }

    pub(crate) fn label(&self) -> Option<&'static str> {
        self.label
    }

    /// Creates a new `LazyGpuHandle` that is already initialized with the given handle.
    pub(crate) fn new_ready(handle: GpuHandle) -> Self {
        Self {
            status: Arc::new(Lock::new(LazyInitStatus::Initialized { handle })),
            label: None,
        }
    }

    pub fn reinit(&self, handle: GpuHandle) {
        let mut status = self.status.write();
        *status = LazyInitStatus::Initialized { handle };
    }

    /// Initializes the underlying GPU resource if it is not already initialized and returns a handle to it.
    /// If the resource is already initialized, this will return a handle to the existing resource without allocating anything new on the GPU.
    pub fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<GpuHandle> {
        let status = self.status.read();
        if let LazyInitStatus::Initialized { handle } = &*status {
            return Ok(handle.clone());
        }
        match &*status {
            LazyInitStatus::Initialized { .. } => unreachable!(),
            LazyInitStatus::Uninitialized {
                ty,
                label,
                pending_data,
            } => match pending_data {
                Some(pending_data) => {
                    let resource = match ty {
                        GpuResourceType::Uniform { usage, .. } => {
                            manager.create_buffer_init(pending_data, *usage, *label)
                        }
                        GpuResourceType::Storage { usage, .. } => {
                            manager.create_buffer_init(pending_data, *usage, *label)
                        }
                        GpuResourceType::Texture {
                            width,
                            height,
                            usage,
                            format,
                            dimension,
                            depth_or_array_layers,
                            ..
                        } => manager.create_texture_init::<_>(
                            *width,
                            *height,
                            *format,
                            *dimension,
                            *depth_or_array_layers,
                            *usage,
                            *label,
                            pending_data,
                        ),
                        GpuResourceType::Sampler {
                            address_mode,
                            filter_mode,
                            compare,
                        } => {
                            log::warn!("Attempted to initialize a sampler with pending data");
                            manager.create_sampler(*address_mode, *filter_mode, *compare, *label)
                        }
                    };

                    // insert the resource into the resource manager to get our handle
                    let handle = manager.insert_resource(resource);

                    // mark the lazy handle as ready
                    drop(status); // unlock
                    *self.status.write() = LazyInitStatus::Initialized {
                        handle: handle.clone(),
                    };
                    Ok(handle)
                }
                None => {
                    let resource = match ty {
                        GpuResourceType::Uniform { usage, size } => {
                            manager.create_buffer(*size, *usage, *label)
                        }
                        GpuResourceType::Storage { usage, size, .. } => {
                            manager.create_buffer(*size, *usage, *label)
                        }
                        GpuResourceType::Texture {
                            width,
                            height,
                            usage,
                            format,
                            dimension,
                            depth_or_array_layers,
                            ..
                        } => manager.create_texture(
                            *width,
                            *height,
                            *format,
                            *dimension,
                            *depth_or_array_layers,
                            *usage,
                            *label,
                        ),
                        GpuResourceType::Sampler {
                            address_mode,
                            filter_mode,
                            compare,
                        } => manager.create_sampler(*address_mode, *filter_mode, *compare, *label),
                    };

                    // insert the resource into the resource manager to get our handle
                    let handle = manager.insert_resource(resource);

                    // mark the lazy handle as ready
                    drop(status); // unlock
                    *self.status.write() = LazyInitStatus::Initialized {
                        handle: handle.clone(),
                    };
                    Ok(handle)
                }
            },
            LazyInitStatus::Destroyed => {
                log::warn!("Attempted to initialize a destroyed GPU resource");
                Err(anyhow::anyhow!("GPU Resource is destroyed"))
            }
        }
    }

    /// Marks the underlying GPU resource as pending an update, if it is not already destroyed.
    /// This will not update the GPU resource until the next frame, unless the render queue is manually flushed.
    /// If the resource has not yet been initialized, this will overwrite the pending data.
    pub fn update<T: bytemuck::Pod>(&self, data: &[T]) {
        let mut status = self.status.write();
        match &mut *status {
            LazyInitStatus::Initialized { handle } => {
                // update the resource
                handle.update(data);
            }
            LazyInitStatus::Uninitialized { pending_data, .. } => {
                // overwrite the pending data
                *pending_data = Some(Arc::from(bytemuck::cast_slice(data)));
            }
            LazyInitStatus::Destroyed => {
                log::warn!("Attempted to update a destroyed buffer");
            }
        }
    }

    /// Marks the underlying GPU resource for destruction, if it is not already destroyed.
    /// This will not destroy the GPU resource until the next frame, unless [`GpuResourceManager::gc_destroyed_resources`] is manually called.
    pub fn mark_destroyed(&self) {
        let mut status = self.status.write();
        match &mut *status {
            LazyInitStatus::Initialized { handle } => {
                handle.mark_destroyed();
            }
            LazyInitStatus::Uninitialized { .. } => {
                *status = LazyInitStatus::Destroyed;
            }
            LazyInitStatus::Destroyed => {
                log::warn!("Attempted to destroy an already destroyed buffer");
            }
        }
    }

    /// Returns true if the handle is initialized and ready to be used.
    pub fn is_initialized(&self) -> bool {
        matches!(&*self.status.read(), LazyInitStatus::Initialized { .. })
    }

    /// Returns true if the handle is pending initialization.
    pub fn is_uninitialized(&self) -> bool {
        matches!(&*self.status.read(), LazyInitStatus::Uninitialized { .. })
    }

    /// Returns true if the handle has been destroyed.
    pub fn is_destroyed(&self) -> bool {
        matches!(&*self.status.read(), LazyInitStatus::Destroyed)
    }
}

/// A lazily initialized bind group.
#[derive(Clone, Debug)]
pub struct LazyBindGroup<T: BindableComponent> {
    /// The bind group layout for the component.
    pub layout: Arc<Lock<Option<Arc<wgpu::BindGroupLayout>>>>,
    /// The bind group for the component.
    pub bind_group: Arc<Lock<Option<Arc<wgpu::BindGroup>>>>,

    _phantom: std::marker::PhantomData<T>,
}

impl<T> Default for LazyBindGroup<T>
where
    T: BindableComponent,
{
    fn default() -> Self {
        Self {
            layout: Arc::new(Lock::new(None)),
            bind_group: Arc::new(Lock::new(None)),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> LazyBindGroup<T>
where
    T: BindableComponent,
{
    /// Returns true if the bind group has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.layout.read().is_some() && self.bind_group.read().is_some()
    }

    /// Resets the bind group and bind group layout for the component.
    /// This is useful for when the component is destroyed and recreated, such as when the window is resized.
    pub fn reset(&self) {
        let mut layout = self.layout.write();
        let mut bind_group = self.bind_group.write();
        *layout = None;
        *bind_group = None;
    }

    /// Returns the bind group for the component, if it has been created.
    pub fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
        self.bind_group.read().as_ref().cloned()
    }

    /// Returns the bind group layout for the component, if it has been created.
    pub fn bind_group_layout(&self) -> Option<Arc<wgpu::BindGroupLayout>> {
        self.layout.read().as_ref().cloned()
    }

    /// Lazily initializes the bind group layout for the component, or returns the existing layout if it has already been initialized.
    pub fn lazy_init_layout(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroupLayout>> {
        let mut layout = self.layout.write();
        if layout.is_none() {
            // create the layout
            *layout = Some(cache.get_or_create::<T>(manager.device()));
        }
        Ok(layout.as_ref().unwrap().clone())
    }

    /// Lazily initializes the bind group for the component, or returns the existing bind group if it has already been initialized.
    pub fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
        component: &T,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        let mut layout = self.layout.write();
        let mut bind_group = self.bind_group.write();
        if layout.is_none() {
            // create the layout
            *layout = Some(cache.get_or_create::<T>(manager.device()));
        }
        if bind_group.is_none() {
            // create the bind group
            *bind_group = Some(component.create_bind_group(manager, cache)?);
        }
        Ok(bind_group.as_ref().unwrap().clone())
    }
}
