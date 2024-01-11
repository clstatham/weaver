use std::{
    cell::RefCell,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
};

use rustc_hash::FxHashMap;
use wgpu::util::DeviceExt;

use super::resource::{GpuHandle, GpuHandleStatus, GpuResource};

/// A manager for GPU-allocated resources.
pub struct GpuResourceManager {
    next_buffer_id: AtomicU64,

    /// The device that the resources are allocated on.
    device: Arc<wgpu::Device>,
    /// The queue that is used to update resources.
    queue: Arc<wgpu::Queue>,

    /// The resources that have been allocated.
    pub(crate) resources: RefCell<FxHashMap<u64, Arc<GpuResource>>>,
    /// The handles to the resources.
    pub(crate) handles: RefCell<FxHashMap<u64, GpuHandle>>,
}

impl GpuResourceManager {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Rc<Self> {
        Rc::new(Self {
            next_buffer_id: AtomicU64::new(0),
            device,
            queue,
            resources: RefCell::new(FxHashMap::default()),
            handles: RefCell::new(FxHashMap::default()),
        })
    }

    /// Returns the device that the resources are allocated on.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Returns the queue that is used to update resources.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Inserts the given resource into the manager and returns a handle to it.
    pub fn insert_resource(&self, buffer: Arc<GpuResource>) -> GpuHandle {
        let id = self
            .next_buffer_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let handle = GpuHandle {
            id,
            status: Rc::new(RefCell::new(GpuHandleStatus::Ready {
                resource: buffer.clone(),
            })),
        };

        self.resources.borrow_mut().insert(id, buffer);
        self.handles.borrow_mut().insert(id, handle.clone());

        handle
    }

    /// Garbage collects any resources that have been destroyed.
    pub fn gc_destroyed_resources(&self) {
        let mut handles = self.handles.borrow_mut();
        let mut buffers = self.resources.borrow_mut();
        let mut destroyed = Vec::new();
        for (id, handle) in handles.iter() {
            if let GpuHandleStatus::Destroyed = &*handle.status.borrow() {
                destroyed.push(*id);
            }
        }
        for id in destroyed {
            handles.remove(&id);
            buffers.remove(&id);
            // buffer is dropped here and destroyed
        }
    }

    /// Creates a GPU-allocated buffer with the given parameters.
    pub fn create_buffer(
        &self,
        size: usize,
        usage: wgpu::BufferUsages,
        label: Option<&'static str>,
    ) -> Arc<GpuResource> {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label,
            size: size as u64,
            usage,
            mapped_at_creation: false,
        });

        Arc::new(GpuResource::Buffer { buffer })
    }

    /// Creates a GPU-allocated buffer with the given parameters and initializes it with the given data.
    pub fn create_buffer_init<T: bytemuck::Pod>(
        &self,
        data: &[T],
        usage: wgpu::BufferUsages,
        label: Option<&'static str>,
    ) -> Arc<GpuResource> {
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::cast_slice(data),
                usage,
            });

        Arc::new(GpuResource::Buffer { buffer })
    }

    /// Creates a GPU-allocated texture with the given parameters.
    pub fn create_texture(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        dimension: wgpu::TextureDimension,
        depth_or_array_layers: u32,
        usage: wgpu::TextureUsages,
        label: Option<&'static str>,
    ) -> Arc<GpuResource> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage,
            view_formats: &[],
        });

        Arc::new(GpuResource::Texture { texture })
    }

    /// Creates a GPU-allocated texture with the given parameters and initializes it with the given data.
    pub fn create_texture_init<T: bytemuck::Pod>(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        dimension: wgpu::TextureDimension,
        depth_or_array_layers: u32,
        usage: wgpu::TextureUsages,
        label: Option<&'static str>,
        data: &[T],
    ) -> Arc<GpuResource> {
        let texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension,
                format,
                usage,
                view_formats: &[],
            },
            bytemuck::cast_slice(data),
        );

        Arc::new(GpuResource::Texture { texture })
    }

    /// Creates a GPU-allocated texture sampler with the given parameters.
    pub fn create_sampler(
        &self,
        address_mode: wgpu::AddressMode,
        filter_mode: wgpu::FilterMode,
        compare: Option<wgpu::CompareFunction>,
        label: Option<&'static str>,
    ) -> Arc<GpuResource> {
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label,
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter: filter_mode,
            min_filter: filter_mode,
            mipmap_filter: filter_mode,
            compare,
            ..Default::default()
        });

        Arc::new(GpuResource::Sampler { sampler })
    }

    /// Updates the resource with the given data.
    /// Returns Ok(()) if the resource was updated successfully, Err otherwise.
    pub fn update_resource(&self, handle: &GpuHandle, pending_data: &[u8]) -> anyhow::Result<()> {
        if let Some(resource) = self.resources.borrow_mut().get_mut(&handle.id) {
            match resource.as_ref() {
                GpuResource::Buffer { ref buffer } => {
                    self.queue.write_buffer(buffer, 0, pending_data);
                }
                GpuResource::Texture { ref texture, .. } => {
                    self.queue.write_texture(
                        texture.as_image_copy(),
                        pending_data,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: None,
                            rows_per_image: None,
                        },
                        texture.size(),
                    );
                }
                _ => {
                    log::warn!("Resource {} is not a buffer or texture", handle.id);
                    return Err(anyhow::anyhow!(
                        "Resource {} is not a buffer or texture",
                        handle.id
                    ));
                }
            }

            Ok(())
        } else {
            log::warn!("Resource {} is missing", handle.id);
            Err(anyhow::anyhow!("Resource {} is missing", handle.id))
        }
    }

    /// Updates all resources that are pending.
    /// This should be called at least once before rendering.
    pub fn update_all_resources(&self) {
        log::trace!("Updating all resources");
        // check for pending resources
        for handle in self.handles.borrow_mut().values_mut() {
            let status = &mut *handle.status.borrow_mut();
            if let GpuHandleStatus::Pending { pending_data } = status {
                // update the resource
                if self.update_resource(handle, pending_data).is_ok() {
                    *status = GpuHandleStatus::Ready {
                        resource: self.resources.borrow().get(&handle.id).unwrap().clone(),
                    };
                }
            }
        }
    }
}
