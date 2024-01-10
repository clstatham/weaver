use std::{
    cell::{Ref, RefCell},
    fmt::Debug,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
};

use egui_wgpu::renderer::ScreenDescriptor;
use rustc_hash::FxHashMap;
use weaver_proc_macro::Resource;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::{
    core::{
        camera::Camera,
        light::{PointLight, PointLightArray},
        material::Material,
        texture::{
            DepthFormat, HdrFormat, NormalMapFormat, PositionMapFormat, TextureFormat, WindowFormat,
        },
        ui::EguiContext,
    },
    ecs::{Component, Query, World},
};

use self::pass::{
    doodads::DoodadRenderPass, gbuffer::GBufferRenderPass, hdr::HdrRenderPass, pbr::PbrRenderPass,
    shadow::OmniShadowRenderPass, sky::SkyRenderPass, Pass,
};

pub mod compute;
pub mod pass;

#[derive(Default)]
pub struct BindGroupLayoutCache {
    /// Bind group layouts for each component id.
    pub(crate) layouts: RefCell<FxHashMap<u64, Arc<wgpu::BindGroupLayout>>>,
}

impl BindGroupLayoutCache {
    pub fn get_or_create<T: Component + CreateBindGroupLayout>(
        &self,
        device: &wgpu::Device,
    ) -> Arc<wgpu::BindGroupLayout> {
        let id = T::component_id();
        if let Some(layout) = self.layouts.borrow().get(&id) {
            return layout.clone();
        }

        let layout = T::create_bind_group_layout(device);
        self.layouts.borrow_mut().insert(id, Arc::new(layout));
        self.layouts.borrow().get(&id).unwrap().clone()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BufferBindingType {
    Uniform {
        usage: wgpu::BufferUsages,
        size: Option<usize>,
    },
    Storage {
        usage: wgpu::BufferUsages,
        size: Option<usize>,
        read_only: bool,
    },
    Texture {
        width: u32,
        height: u32,
        usage: wgpu::TextureUsages,
        format: wgpu::TextureFormat,
        dimension: wgpu::TextureDimension,
        view_dimension: wgpu::TextureViewDimension,
        depth_or_array_layers: u32,
    },
}

impl Into<wgpu::BindingType> for &BufferBindingType {
    fn into(self) -> wgpu::BindingType {
        match self {
            BufferBindingType::Uniform { .. } => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            BufferBindingType::Storage { read_only, .. } => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: *read_only,
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            BufferBindingType::Texture { view_dimension, .. } => wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: *view_dimension,
                multisampled: false,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct LazyBufferHandle {
    handle: Rc<RefCell<Option<BufferHandle>>>,
    pub ty: BufferBindingType,
    pub label: Option<&'static str>,
    pending_data: Rc<RefCell<Option<Arc<[u8]>>>>,
}

impl LazyBufferHandle {
    pub fn new(
        ty: BufferBindingType,
        label: Option<&'static str>,
        pending_data: Option<Arc<[u8]>>,
    ) -> Self {
        Self {
            handle: Rc::new(RefCell::new(None)),
            ty,
            label,
            pending_data: Rc::new(RefCell::new(pending_data)),
        }
    }

    pub fn from_handle(
        handle: BufferHandle,
        ty: BufferBindingType,
        label: Option<&'static str>,
    ) -> Self {
        Self {
            handle: Rc::new(RefCell::new(Some(handle))),
            ty,
            label,
            pending_data: Rc::new(RefCell::new(None)),
        }
    }

    pub fn get_or_create<C: Component + CreateBindGroupLayout>(
        &self,
        renderer: &Renderer,
    ) -> BufferHandle {
        if let Some(handle) = self.handle.borrow().as_ref().cloned() {
            return handle;
        }
        if let Some(data) = self.pending_data.borrow_mut().take() {
            return self.get_or_create_init::<_, C>(renderer, &data);
        }
        let buffer = match self.ty {
            BufferBindingType::Uniform { usage, size } => renderer.create_buffer::<C>(
                size.unwrap_or(std::mem::size_of::<glam::Mat4>()),
                usage,
                self.label,
            ),
            BufferBindingType::Storage { usage, size, .. } => renderer.create_buffer::<C>(
                size.unwrap_or(std::mem::size_of::<glam::Mat4>()),
                usage,
                self.label,
            ),
            BufferBindingType::Texture {
                width,
                height,
                usage,
                format,
                dimension,
                view_dimension,
                depth_or_array_layers,
            } => renderer.create_texture::<C>(
                width,
                height,
                format,
                dimension,
                view_dimension,
                depth_or_array_layers,
                usage,
                self.label,
            ),
        };

        let handle = renderer.buffer_allocator.insert_buffer(buffer);

        *self.handle.borrow_mut() = Some(handle.clone());
        handle
    }

    pub fn get_or_create_init<T: bytemuck::Pod, C: Component + CreateBindGroupLayout>(
        &self,
        renderer: &Renderer,
        data: &[T],
    ) -> BufferHandle {
        if let Some(handle) = self.handle.borrow().as_ref().cloned() {
            return handle;
        }
        let buffer = match self.ty {
            BufferBindingType::Uniform { usage, .. } => {
                renderer.create_buffer_init::<T, C>(data, usage, self.label)
            }
            BufferBindingType::Storage { usage, .. } => {
                renderer.create_buffer_init::<T, C>(data, usage, self.label)
            }
            BufferBindingType::Texture {
                width,
                height,
                usage,
                format,
                dimension,
                view_dimension,
                depth_or_array_layers,
            } => renderer.create_texture_init::<_, C>(
                width,
                height,
                format,
                dimension,
                view_dimension,
                depth_or_array_layers,
                usage,
                self.label,
                data,
            ),
        };

        let handle = renderer.buffer_allocator.insert_buffer(buffer);

        *self.handle.borrow_mut() = Some(handle.clone());
        handle
    }

    pub fn update<T: bytemuck::Pod>(&self, data: &[T]) {
        if let Some(handle) = self.handle.borrow_mut().as_mut() {
            handle.update(data);
        }
    }
}

impl Drop for LazyBufferHandle {
    fn drop(&mut self) {
        if let Some(mut handle) = self.handle.borrow_mut().take() {
            handle.destroy();
        }
    }
}

#[derive(Clone)]
pub enum UpdateStatus {
    Ready { buffer: Arc<BindableBuffer> },
    Pending { pending_data: Arc<[u8]> },
    Destroyed,
}

impl UpdateStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, UpdateStatus::Ready { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, UpdateStatus::Pending { .. })
    }

    pub fn is_destroyed(&self) -> bool {
        matches!(self, UpdateStatus::Destroyed)
    }
}

impl Debug for UpdateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateStatus::Ready { .. } => write!(f, "Ready"),
            UpdateStatus::Pending { .. } => write!(f, "Pending"),
            UpdateStatus::Destroyed => write!(f, "Destroyed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BufferHandle {
    id: u64,
    status: Rc<RefCell<UpdateStatus>>,
}

pub enum BufferStorage {
    Buffer {
        buffer: wgpu::Buffer,
    },
    Texture {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
    },
}

#[derive(Clone)]
pub struct BindableBuffer {
    pub storage: Arc<BufferStorage>,
    pub bind_group: Arc<wgpu::BindGroup>,
}

impl BufferHandle {
    /// Marks the buffer as pending an update.
    /// This does NOT write to the buffer immediately.
    pub fn update<T: bytemuck::Pod>(&mut self, data: &[T]) {
        let mut status = self.status.borrow_mut();
        if !status.is_destroyed() {
            *status = UpdateStatus::Pending {
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

    pub fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
        if let UpdateStatus::Ready { ref buffer } = &*self.status.borrow() {
            Some(buffer.bind_group.clone())
        } else {
            log::warn!(
                "Attempted to get bind group for buffer that is not ready: {} is {:?}",
                self.id,
                self.status
            );
            None
        }
    }

    pub fn get(&self) -> Option<Arc<BufferStorage>> {
        if let UpdateStatus::Ready { ref buffer } = &*self.status.borrow() {
            Some(buffer.storage.clone())
        } else {
            log::warn!(
                "Attempted to get buffer that is not ready: {} is {:?}",
                self.id,
                self.status
            );
            None
        }
    }

    pub fn get_buffer(&self) -> Option<Ref<'_, wgpu::Buffer>> {
        let status = self.status.borrow();
        if let UpdateStatus::Ready { ref buffer } = &*status {
            match buffer.storage.as_ref() {
                BufferStorage::Buffer { .. } => Some(Ref::map(status, |status| match status {
                    UpdateStatus::Ready { buffer } => match buffer.storage.as_ref() {
                        BufferStorage::Buffer { buffer } => buffer,
                        BufferStorage::Texture { .. } => panic!("Buffer is not ready"),
                    },
                    _ => panic!("Buffer is not ready"),
                })),
                BufferStorage::Texture { .. } => {
                    log::warn!(
                        "Attempted to get buffer from texture: {} is {:?}",
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

    pub fn get_texture(&self) -> Option<Ref<'_, wgpu::Texture>> {
        let status = self.status.borrow();
        if let UpdateStatus::Ready { ref buffer } = &*status {
            match buffer.storage.as_ref() {
                BufferStorage::Buffer { .. } => {
                    log::warn!(
                        "Attempted to get texture from buffer: {} is {:?}",
                        self.id,
                        self.status
                    );
                    None
                }
                BufferStorage::Texture { .. } => Some(Ref::map(status, |status| match status {
                    UpdateStatus::Ready { buffer } => match buffer.storage.as_ref() {
                        BufferStorage::Buffer { .. } => panic!("Texture is not ready"),
                        BufferStorage::Texture { texture, .. } => texture,
                    },
                    _ => panic!("Texture is not ready"),
                })),
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

    pub fn destroy(&mut self) {
        let mut status = self.status.borrow_mut();
        match &mut *status {
            UpdateStatus::Ready { .. } => {
                *status = UpdateStatus::Destroyed;
            }
            UpdateStatus::Pending { .. } => {
                *status = UpdateStatus::Destroyed;
            }
            UpdateStatus::Destroyed => {
                log::warn!("Attempted to destroy buffer that is already destroyed");
            }
        }
    }
}

pub trait CreateBindGroupLayout {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout;
}

#[derive(Component)]
pub struct NonFilteringSampler;

impl CreateBindGroupLayout for NonFilteringSampler {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Non Filtering Sampler Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            }],
        })
    }
}

#[derive(Component)]
pub struct NearestSampler;

impl CreateBindGroupLayout for NearestSampler {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nearest Sampler Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            }],
        })
    }
}

#[derive(Component)]
pub struct ComparisonSampler;

impl CreateBindGroupLayout for ComparisonSampler {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Comparison Sampler Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            }],
        })
    }
}

pub trait HasBindGroup {
    fn get_or_create_bind_group(&self, renderer: &Renderer) -> Arc<wgpu::BindGroup>;
}

pub trait AllocBuffers {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>>;
}

pub struct BufferAllocator {
    next_buffer_id: AtomicU64,

    pub(crate) buffers: RefCell<FxHashMap<u64, Arc<BindableBuffer>>>,
    pub(crate) buffer_handles: RefCell<FxHashMap<u64, BufferHandle>>,
}

impl BufferAllocator {
    pub fn insert_buffer(&self, buffer: Arc<BindableBuffer>) -> BufferHandle {
        let id = self
            .next_buffer_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let handle = BufferHandle {
            id,
            status: Rc::new(RefCell::new(UpdateStatus::Ready {
                buffer: buffer.clone(),
            })),
        };

        self.buffers.borrow_mut().insert(id, buffer);
        self.buffer_handles.borrow_mut().insert(id, handle.clone());

        handle
    }

    pub fn gc_destroyed_buffers(&self) {
        let mut handles = self.buffer_handles.borrow_mut();
        let mut buffers = self.buffers.borrow_mut();
        let mut destroyed = Vec::new();
        for (id, handle) in handles.iter() {
            if let UpdateStatus::Destroyed = &*handle.status.borrow() {
                destroyed.push(*id);
            }
        }
        for id in destroyed {
            handles.remove(&id);
            buffers.remove(&id);
            // buffer is dropped here and destroyed
        }
    }
}

#[derive(Resource)]
#[allow(dead_code)]
pub struct Renderer {
    pub(crate) surface: wgpu::Surface,
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    pub(crate) config: wgpu::SurfaceConfiguration,

    pub(crate) color_texture: wgpu::Texture,
    pub(crate) color_texture_view: wgpu::TextureView,
    pub(crate) depth_texture: wgpu::Texture,
    pub(crate) depth_texture_view: wgpu::TextureView,
    pub(crate) position_texture: wgpu::Texture,
    pub(crate) position_texture_view: wgpu::TextureView,
    pub(crate) normal_texture: wgpu::Texture,
    pub(crate) normal_texture_view: wgpu::TextureView,

    pub(crate) hdr_pass: HdrRenderPass,
    pub(crate) gbuffer_pass: GBufferRenderPass,
    pub(crate) pbr_pass: PbrRenderPass,
    pub(crate) sky_pass: SkyRenderPass,
    pub shadow_pass: OmniShadowRenderPass,
    pub doodad_pass: DoodadRenderPass,
    pub(crate) extra_passes: Vec<Box<dyn pass::Pass>>,

    pub(crate) buffer_allocator: Rc<BufferAllocator>,
    pub(crate) bind_group_layout_cache: BindGroupLayoutCache,

    pub(crate) point_lights: PointLightArray,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = unsafe { instance.create_surface(window) }.unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::all_webgpu_mask() | wgpu::Features::MULTIVIEW,
                limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        ))
        .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: WindowFormat::FORMAT,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Color Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WindowFormat::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let color_texture_view = color_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Color Texture View"),
            format: Some(WindowFormat::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DepthFormat::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Depth Texture View"),
            format: Some(DepthFormat::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });

        let position_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Position Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PositionMapFormat::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let position_texture_view = position_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Position Texture View"),
            format: Some(PositionMapFormat::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });

        let normal_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Normal Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: NormalMapFormat::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let normal_texture_view = normal_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Normal Texture View"),
            format: Some(NormalMapFormat::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });

        let buffer_allocator = Rc::new(BufferAllocator {
            next_buffer_id: AtomicU64::new(0),
            buffers: RefCell::new(FxHashMap::default()),
            buffer_handles: RefCell::new(FxHashMap::default()),
        });

        let bind_group_layout_cache = BindGroupLayoutCache::default();

        let hdr_pass = HdrRenderPass::new(
            &device,
            config.width,
            config.height,
            &bind_group_layout_cache,
        );

        let gbuffer_pass = GBufferRenderPass::new(&device, &bind_group_layout_cache);

        let pbr_pass = PbrRenderPass::new(&device, &bind_group_layout_cache);

        let sky_pass = SkyRenderPass::new(&device, &bind_group_layout_cache);

        // let shadow_pass = ShadowRenderPass::new(&device, &sampler_clamp_nearest, &sampler_depth);

        let shadow_pass = OmniShadowRenderPass::new(&device, &bind_group_layout_cache);

        let doodad_pass = DoodadRenderPass::new(&device, &config, &bind_group_layout_cache);

        let extra_passes: Vec<Box<dyn Pass>> = vec![];

        Self {
            surface,
            device: Arc::new(device),
            queue: Arc::new(queue),
            config,
            color_texture,
            color_texture_view,
            depth_texture,
            depth_texture_view,
            position_texture,
            position_texture_view,
            normal_texture,
            normal_texture_view,
            hdr_pass,
            gbuffer_pass,
            pbr_pass,
            shadow_pass,
            sky_pass,
            doodad_pass,
            extra_passes,
            buffer_allocator,
            bind_group_layout_cache,
            point_lights: PointLightArray::new(),
        }
    }

    pub fn create_buffer<C: Component + CreateBindGroupLayout>(
        &self,
        size: usize,
        usage: wgpu::BufferUsages,
        label: Option<&'static str>,
    ) -> Arc<BindableBuffer> {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label,
            size: size as u64,
            usage,
            mapped_at_creation: false,
        });

        let bind_group_layout = self
            .bind_group_layout_cache
            .get_or_create::<C>(self.device.as_ref());

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Buffer Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Arc::new(BindableBuffer {
            storage: Arc::new(BufferStorage::Buffer { buffer }),
            bind_group: Arc::new(bind_group),
        })
    }

    pub fn create_buffer_init<T: bytemuck::Pod, C: Component + CreateBindGroupLayout>(
        &self,
        data: &[T],
        usage: wgpu::BufferUsages,
        label: Option<&'static str>,
    ) -> Arc<BindableBuffer> {
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::cast_slice(data),
                usage,
            });

        let bind_group_layout = self
            .bind_group_layout_cache
            .get_or_create::<C>(self.device.as_ref());

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Buffer Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Arc::new(BindableBuffer {
            storage: Arc::new(BufferStorage::Buffer { buffer }),
            bind_group: Arc::new(bind_group),
        })
    }

    pub fn create_texture<F: Component + CreateBindGroupLayout>(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        dimension: wgpu::TextureDimension,
        view_dimension: wgpu::TextureViewDimension,
        depth_or_array_layers: u32,
        usage: wgpu::TextureUsages,
        label: Option<&'static str>,
    ) -> Arc<BindableBuffer> {
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

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Texture View"),
            format: Some(format),
            dimension: Some(view_dimension),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: Some(depth_or_array_layers),
            mip_level_count: None,
        });

        let bind_group_layout = self
            .bind_group_layout_cache
            .get_or_create::<F>(&self.device);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });

        Arc::new(BindableBuffer {
            storage: Arc::new(BufferStorage::Texture { texture, view }),
            bind_group: Arc::new(bind_group),
        })
    }

    pub fn create_texture_init<T: bytemuck::Pod, F: Component + CreateBindGroupLayout>(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        dimension: wgpu::TextureDimension,
        view_dimension: wgpu::TextureViewDimension,
        depth_or_array_layers: u32,
        usage: wgpu::TextureUsages,
        label: Option<&'static str>,
        data: &[T],
    ) -> Arc<BindableBuffer> {
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

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Texture View"),
            format: Some(format),
            dimension: Some(view_dimension),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });

        let bind_group_layout = self
            .bind_group_layout_cache
            .get_or_create::<F>(&self.device);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });

        Arc::new(BindableBuffer {
            storage: Arc::new(BufferStorage::Texture { texture, view }),
            bind_group: Arc::new(bind_group),
        })
    }

    /// Updates the buffer with the given data.
    /// Returns true if the buffer was updated, false if the buffer is missing.
    pub fn update_buffer(&self, handle: &BufferHandle, pending_data: &[u8]) -> bool {
        if let Some(buffer) = self
            .buffer_allocator
            .buffers
            .borrow_mut()
            .get_mut(&handle.id)
        {
            match buffer.storage.as_ref() {
                BufferStorage::Buffer { ref buffer } => {
                    self.queue.write_buffer(buffer, 0, pending_data);
                }
                BufferStorage::Texture { ref texture, .. } => {
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
            }

            true
        } else {
            log::warn!("Buffer {} is missing", handle.id);
            false
        }
    }

    /// Updates all buffers that are pending.
    /// This should be called at least once before rendering.
    pub fn update_all_buffers(&mut self) {
        log::trace!("Updating all buffers");
        for handle in self
            .buffer_allocator
            .buffer_handles
            .borrow_mut()
            .values_mut()
        {
            let status = &mut *handle.status.borrow_mut();
            if let UpdateStatus::Pending { pending_data } = status {
                if self.update_buffer(handle, pending_data) {
                    *status = UpdateStatus::Ready {
                        buffer: self
                            .buffer_allocator
                            .buffers
                            .borrow()
                            .get(&handle.id)
                            .unwrap()
                            .clone(),
                    };
                }
            }
        }
    }

    pub fn force_flush(&self) {
        log::trace!("Forcing flush of render queue");
        self.queue.submit(std::iter::once(
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Force Flush Encoder"),
                })
                .finish(),
        ));
    }

    /// Flushes the render queue, submitting the given encoder and marking all buffers as ready.
    pub fn flush(&self, encoder: wgpu::CommandEncoder) {
        log::trace!("Flushing render queue");
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn push_render_pass<T: Pass + 'static>(&mut self, pass: T) {
        self.extra_passes.push(Box::new(pass));
    }

    pub fn prepare_components(&mut self, world: &World) {
        log::trace!("Preparing components");
        // prepare the renderer's built-in components
        self.hdr_pass
            .texture
            .handle
            .get_or_create::<HdrFormat>(self);
        self.point_lights.alloc_buffers(self).unwrap();

        // query the world for the types that need to allocate buffers
        // these are currently:
        // - Material
        // - PointLight
        // - Camera

        {
            let query = Query::<&Material>::new(world);
            for material in query.iter() {
                material.alloc_buffers(self).unwrap();
                material.update(self).unwrap();
            }
        }

        {
            self.point_lights.clear();

            let query = Query::<&PointLight>::new(world);
            for light in query.iter() {
                light.alloc_buffers(self).unwrap();
                self.point_lights.add_light(&light);
            }

            self.point_lights.update();
        }

        {
            let query = Query::<&Camera>::new(world);
            for camera in query.iter() {
                camera.alloc_buffers(self).unwrap();
            }
        }

        self.update_all_buffers();
    }

    pub fn render_ui(
        &self,
        ui: &mut EguiContext,
        window: &Window,
        output: &wgpu::SurfaceTexture,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("UI Texture View"),
            format: Some(WindowFormat::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });

        ui.render(
            &self.device,
            &self.queue,
            encoder,
            window,
            &view,
            &ScreenDescriptor {
                size_in_pixels: [self.config.width, self.config.height],
                pixels_per_point: window.scale_factor() as f32,
            },
        );
    }

    pub fn render(
        &mut self,
        world: &World,
        output: &wgpu::SurfaceTexture,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let hdr_pass_view = {
            let hdr_pass_handle = &self
                .hdr_pass
                .texture
                .handle
                .get_or_create::<HdrFormat>(self);
            let hdr_pass_texture = hdr_pass_handle.get_texture().unwrap();
            hdr_pass_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("HDR Pass Texture View"),
                format: Some(HdrFormat::FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                base_array_layer: 0,
                array_layer_count: None,
                mip_level_count: None,
            })
        };

        // clear the screen
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Screen"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &hdr_pass_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.normal_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.position_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        self.gbuffer_pass.render_if_enabled(
            encoder,
            &hdr_pass_view,
            &self.depth_texture_view,
            self,
            world,
        )?;

        self.pbr_pass.render(self, &hdr_pass_view, world, encoder)?;

        for pass in self.extra_passes.iter() {
            pass.render_if_enabled(
                encoder,
                &hdr_pass_view,
                &self.depth_texture_view,
                self,
                world,
            )?;
        }

        self.sky_pass.render_if_enabled(
            encoder,
            &hdr_pass_view,
            &self.depth_texture_view,
            self,
            world,
        )?;

        // we always want to render the HDR pass, otherwise we won't see anything!
        self.hdr_pass.render(
            encoder,
            &self.color_texture_view,
            &self.depth_texture_view,
            self,
            world,
        )?;

        self.shadow_pass.render_if_enabled(
            encoder,
            &self.color_texture_view,
            &self.depth_texture_view,
            self,
            world,
        )?;

        self.doodad_pass.render_if_enabled(
            encoder,
            &self.color_texture_view,
            &self.depth_texture_view,
            self,
            world,
        )?;

        // self.particle_pass.render_if_enabled(
        //     &self.device,
        //     &self.queue,
        //     &self.color_texture_view,
        //     &self.depth_texture_view,
        //     self,
        //     world,
        // )?;

        // copy color texture to the output
        encoder.copy_texture_to_texture(
            self.color_texture.as_image_copy(),
            output.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    pub fn begin_frame(&mut self) -> (wgpu::SurfaceTexture, wgpu::CommandEncoder) {
        log::trace!("Begin frame");

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main Render Encoder"),
            });

        let output = self.surface.get_current_texture().unwrap();

        (output, encoder)
    }

    pub fn prepare_passes(&mut self, world: &World) {
        log::trace!("Preparing passes");
        self.gbuffer_pass.prepare(world, self).unwrap();
        self.pbr_pass.prepare(world, self);
        self.shadow_pass.prepare(world, self).unwrap();
        self.doodad_pass.prepare(world, self).unwrap();
        self.sky_pass.prepare(world, self).unwrap();
        self.hdr_pass.prepare(world, self).unwrap();
        for pass in self.extra_passes.iter() {
            pass.prepare(world, self).unwrap();
        }
        self.update_all_buffers();
    }

    pub fn flush_and_present(&self, output: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.flush(encoder);
        output.present();
        self.buffer_allocator.gc_destroyed_buffers();
    }
}
