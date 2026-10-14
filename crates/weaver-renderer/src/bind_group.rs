//! Bind groups built from a single declarative description.
//!
//! A type implements [`AsBindGroup`] once, listing its bindings in order. The same description
//! produces the [`BindGroupLayout`] (via [`BindGroupLayoutBuilder`]) and the bind group itself (via
//! [`BindingsBuilder`]), and the two are checked against each other, so a layout and its bind
//! groups can't silently drift apart.
//!
//! A [`BindGroup<T>`] remembers which GPU resources it was built from. When one of them is
//! recreated (for example a [`GpuBufferVec`] that grew), [`BindGroup::is_outdated`] notices, and
//! the bind group plugins rebuild it. Nobody has to remember to mark anything stale.

use std::{any::TypeId, marker::PhantomData, ops::Deref, sync::Arc};

use encase::{ShaderType, internal::WriteInto};
use weaver_app::{App, plugin::Plugin};
use weaver_ecs::{
    change_detection::ChangeDetection,
    commands::Commands,
    component::Res,
    condition::{not, resource_exists},
    entity::Entity,
    prelude::Component,
    query::Query,
    removal::RemovedComponents,
    system::IntoSystemConfig,
};
use weaver_util::prelude::*;

use crate::{
    GpuResourceId, RenderStage, WgpuDevice,
    buffer::{GpuBuffer, GpuBufferVec},
    texture::GpuTexture,
};

/// The kind of resource a binding slot holds. Bindings are checked against their layout by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    /// A uniform or storage buffer.
    Buffer,
    /// A sampled texture.
    Texture,
    /// A storage texture.
    StorageTexture,
    /// A sampler.
    Sampler,
    /// Anything else (such as an acceleration structure). In a layout, matches any binding.
    Other,
}

impl BindingKind {
    fn of(ty: &wgpu::BindingType) -> Self {
        match ty {
            wgpu::BindingType::Buffer { .. } => Self::Buffer,
            wgpu::BindingType::Texture { .. } => Self::Texture,
            wgpu::BindingType::StorageTexture { .. } => Self::StorageTexture,
            wgpu::BindingType::Sampler(_) => Self::Sampler,
            _ => Self::Other,
        }
    }
}

/// Builds a bind group layout. Each call adds the next binding (0, 1, 2, ...).
pub struct BindGroupLayoutBuilder {
    entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl BindGroupLayoutBuilder {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Adds a binding of any type.
    pub fn entry(&mut self, visibility: wgpu::ShaderStages, ty: wgpu::BindingType) -> &mut Self {
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding: self.entries.len() as u32,
            visibility,
            ty,
            count: None,
        });
        self
    }

    /// A uniform buffer.
    pub fn uniform_buffer(&mut self, visibility: wgpu::ShaderStages) -> &mut Self {
        self.entry(
            visibility,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        )
    }

    /// A storage buffer.
    pub fn storage_buffer(&mut self, visibility: wgpu::ShaderStages, read_only: bool) -> &mut Self {
        self.entry(
            visibility,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        )
    }

    /// A non-multisampled texture with the given dimension and sample type.
    pub fn texture(
        &mut self,
        visibility: wgpu::ShaderStages,
        view_dimension: wgpu::TextureViewDimension,
        sample_type: wgpu::TextureSampleType,
    ) -> &mut Self {
        self.entry(
            visibility,
            wgpu::BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled: false,
            },
        )
    }

    /// A filterable float 2D texture.
    pub fn texture_2d(&mut self, visibility: wgpu::ShaderStages) -> &mut Self {
        self.texture(
            visibility,
            wgpu::TextureViewDimension::D2,
            wgpu::TextureSampleType::Float { filterable: true },
        )
    }

    /// A filterable float cube texture.
    pub fn texture_cube(&mut self, visibility: wgpu::ShaderStages) -> &mut Self {
        self.texture(
            visibility,
            wgpu::TextureViewDimension::Cube,
            wgpu::TextureSampleType::Float { filterable: true },
        )
    }

    /// A storage texture.
    pub fn storage_texture(
        &mut self,
        visibility: wgpu::ShaderStages,
        access: wgpu::StorageTextureAccess,
        format: wgpu::TextureFormat,
        view_dimension: wgpu::TextureViewDimension,
    ) -> &mut Self {
        self.entry(
            visibility,
            wgpu::BindingType::StorageTexture {
                access,
                format,
                view_dimension,
            },
        )
    }

    /// A sampler of the given type.
    pub fn sampler(
        &mut self,
        visibility: wgpu::ShaderStages,
        ty: wgpu::SamplerBindingType,
    ) -> &mut Self {
        self.entry(visibility, wgpu::BindingType::Sampler(ty))
    }

    /// A filtering sampler.
    pub fn filtering_sampler(&mut self, visibility: wgpu::ShaderStages) -> &mut Self {
        self.sampler(visibility, wgpu::SamplerBindingType::Filtering)
    }

    /// The entries added so far.
    pub fn entries(&self) -> &[wgpu::BindGroupLayoutEntry] {
        &self.entries
    }
}

/// A bind group layout, plus the binding kinds it was built with.
#[derive(Clone)]
pub struct BindGroupLayout {
    layout: Arc<wgpu::BindGroupLayout>,
    kinds: Arc<[BindingKind]>,
    label: &'static str,
}

impl BindGroupLayout {
    /// Creates a layout; `build` adds its bindings.
    pub fn new(
        device: &wgpu::Device,
        label: &'static str,
        build: impl FnOnce(&mut BindGroupLayoutBuilder),
    ) -> Self {
        let mut builder = BindGroupLayoutBuilder::new();
        build(&mut builder);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &builder.entries,
        });
        Self {
            layout: Arc::new(layout),
            kinds: builder
                .entries
                .iter()
                .map(|entry| BindingKind::of(&entry.ty))
                .collect(),
            label,
        }
    }

    /// The layout described by `T`'s [`AsBindGroup`] impl. Prefer [`BindGroupLayouts::get`], which
    /// caches it.
    pub fn of<T: AsBindGroup>(device: &wgpu::Device) -> Self {
        Self::new(device, T::LABEL, T::layout)
    }

    /// The layout's label (the `LABEL` of the type it came from).
    pub fn label(&self) -> &'static str {
        self.label
    }
}

impl Deref for BindGroupLayout {
    type Target = wgpu::BindGroupLayout;

    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

/// One layout per [`AsBindGroup`] type, created on first use.
#[derive(Default)]
pub struct BindGroupLayouts {
    cache: Lock<TypeIdMap<BindGroupLayout>>,
}

impl BindGroupLayouts {
    /// The layout for `T`, creating it if this is the first request.
    pub fn get<T: AsBindGroup>(&self, device: &wgpu::Device) -> BindGroupLayout {
        if let Some(layout) = self.cache.read().get(&TypeId::of::<T>()) {
            return layout.clone();
        }
        self.cache
            .write()
            .entry(TypeId::of::<T>())
            .or_insert_with(|| BindGroupLayout::of::<T>(device))
            .clone()
    }
}

/// Something that can be bound as a bind group.
///
/// `layout` and `bindings` must list the same bindings in the same order; [`BindGroup::new`]
/// panics if their kinds don't line up.
///
/// To get a [`BindGroup<T>`] kept up to date automatically, add a [`ComponentBindGroupPlugin`]
/// (for a component) or a [`ResourceBindGroupPlugin`] (for a resource).
///
/// # Examples
///
/// ```
/// use weaver_renderer::{bind_group::*, buffer::GpuBufferVec};
///
/// struct Lights {
///     buffer: GpuBufferVec<[f32; 4]>,
/// }
///
/// impl AsBindGroup for Lights {
///     const LABEL: &'static str = "Lights";
///
///     fn layout(layout: &mut BindGroupLayoutBuilder) {
///         layout.storage_buffer(wgpu::ShaderStages::FRAGMENT, true);
///     }
///
///     fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
///         bindings.buffer_vec(&self.buffer);
///     }
/// }
/// ```
pub trait AsBindGroup: Send + Sync + 'static {
    /// Label for the layout and bind group, shown in GPU debuggers and validation errors.
    const LABEL: &'static str;

    /// Declares the bindings, in order.
    fn layout(layout: &mut BindGroupLayoutBuilder);

    /// Lists the resources for those bindings, in the same order as [`AsBindGroup::layout`].
    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>);
}

/// What a binding was built from, for telling whether a bind group is out of date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingSource {
    /// A tracked GPU resource; `size` is the bound byte size for buffers.
    Tracked { id: GpuResourceId, size: u64 },
    /// A raw wgpu resource; changes to it can't be detected.
    Untracked,
}

/// Collects a bind group's resources in binding order. See [`AsBindGroup`].
pub struct BindingsBuilder<'a> {
    resources: Vec<wgpu::BindingResource<'a>>,
    kinds: Vec<BindingKind>,
    sources: Vec<BindingSource>,
}

impl<'a> BindingsBuilder<'a> {
    fn new() -> Self {
        Self {
            resources: Vec::new(),
            kinds: Vec::new(),
            sources: Vec::new(),
        }
    }

    fn push(
        &mut self,
        resource: wgpu::BindingResource<'a>,
        kind: BindingKind,
        source: BindingSource,
    ) -> &mut Self {
        self.resources.push(resource);
        self.kinds.push(kind);
        self.sources.push(source);
        self
    }

    /// Binds a whole buffer.
    pub fn buffer(&mut self, buffer: &'a GpuBuffer) -> &mut Self {
        let source = BindingSource::Tracked {
            id: buffer.id(),
            size: buffer.size(),
        };
        self.push(
            wgpu::BindingResource::Buffer(buffer.as_entire_buffer_binding()),
            BindingKind::Buffer,
            source,
        )
    }

    /// Binds the uploaded elements of a [`GpuBufferVec`].
    ///
    /// # Panics
    ///
    /// Panics if nothing has been uploaded yet, since an empty buffer binding isn't allowed.
    pub fn buffer_vec<T: ShaderType + WriteInto>(
        &mut self,
        buffer: &'a GpuBufferVec<T>,
    ) -> &mut Self {
        let (Some(gpu_buffer), Some(binding)) = (buffer.buffer(), buffer.binding()) else {
            panic!(
                "binding {}: GpuBufferVec<{}> has no uploaded elements; upload at least one before binding it",
                self.resources.len(),
                std::any::type_name::<T>()
            );
        };
        let source = BindingSource::Tracked {
            id: gpu_buffer.id(),
            size: (buffer.uploaded_len() as u64) * u64::from(T::min_size()),
        };
        self.push(binding, BindingKind::Buffer, source)
    }

    /// Binds a texture's default view.
    pub fn texture(&mut self, texture: &'a GpuTexture) -> &mut Self {
        let source = BindingSource::Tracked {
            id: texture.id(),
            size: 0,
        };
        self.push(
            wgpu::BindingResource::TextureView(&texture.view),
            BindingKind::Texture,
            source,
        )
    }

    /// Binds a texture's default view as a storage texture.
    pub fn storage_texture(&mut self, texture: &'a GpuTexture) -> &mut Self {
        let source = BindingSource::Tracked {
            id: texture.id(),
            size: 0,
        };
        self.push(
            wgpu::BindingResource::TextureView(&texture.view),
            BindingKind::StorageTexture,
            source,
        )
    }

    /// Binds a raw texture view. Prefer [`BindingsBuilder::texture`]: a raw view being replaced
    /// can't be detected.
    pub fn texture_view(&mut self, view: &'a wgpu::TextureView) -> &mut Self {
        // TODO: raw views are never detected as changed (only GpuBuffer/GpuTexture bindings are
        // tracked)
        self.push(
            wgpu::BindingResource::TextureView(view),
            BindingKind::Texture,
            BindingSource::Untracked,
        )
    }

    /// Binds a sampler. Like raw texture views, samplers aren't tracked for changes.
    pub fn sampler(&mut self, sampler: &'a wgpu::Sampler) -> &mut Self {
        self.push(
            wgpu::BindingResource::Sampler(sampler),
            BindingKind::Sampler,
            BindingSource::Untracked,
        )
    }
}

/// A bind group for `T`, created from `T`'s [`AsBindGroup`] impl.
pub struct BindGroup<T: AsBindGroup> {
    bind_group: Arc<wgpu::BindGroup>,
    sources: Vec<BindingSource>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: AsBindGroup> BindGroup<T> {
    /// Creates the bind group from `data`'s current bindings.
    ///
    /// # Panics
    ///
    /// Panics if `data`'s bindings don't match the layout in count or kind.
    pub fn new(device: &wgpu::Device, layout: &BindGroupLayout, data: &T) -> Self {
        let mut bindings = BindingsBuilder::new();
        data.bindings(&mut bindings);

        assert_eq!(
            bindings.kinds.len(),
            layout.kinds.len(),
            "{}: AsBindGroup::bindings produced {} bindings, but layout `{}` has {}",
            std::any::type_name::<T>(),
            bindings.kinds.len(),
            layout.label,
            layout.kinds.len(),
        );
        for (index, (given, expected)) in bindings.kinds.iter().zip(layout.kinds.iter()).enumerate()
        {
            assert!(
                given == expected || *expected == BindingKind::Other,
                "{}: binding {index} is a {given:?}, but layout `{}` expects a {expected:?}",
                std::any::type_name::<T>(),
                layout.label,
            );
        }

        let entries = bindings
            .resources
            .into_iter()
            .enumerate()
            .map(|(binding, resource)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource,
            })
            .collect::<Vec<_>>();
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(T::LABEL),
            layout,
            entries: &entries,
        });

        Self {
            bind_group: Arc::new(bind_group),
            sources: bindings.sources,
            _marker: PhantomData,
        }
    }

    /// True if one of the tracked resources `data` binds was replaced or resized since this bind
    /// group was created.
    pub fn is_outdated(&self, data: &T) -> bool {
        let mut bindings = BindingsBuilder::new();
        data.bindings(&mut bindings);
        bindings.sources != self.sources
    }

    /// The underlying wgpu bind group, cheap to clone.
    pub fn bind_group(&self) -> &Arc<wgpu::BindGroup> {
        &self.bind_group
    }
}

impl<T: AsBindGroup> Deref for BindGroup<T> {
    type Target = wgpu::BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.bind_group
    }
}

/// Keeps a `BindGroup<T>` component next to every `T` component: created when `T` appears,
/// rebuilt when the resources `T` binds are replaced, removed when `T` is removed.
pub struct ComponentBindGroupPlugin<T: Component + AsBindGroup>(PhantomData<T>);

impl<T: Component + AsBindGroup> Default for ComponentBindGroupPlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Component + AsBindGroup> Plugin for ComponentBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(
            prepare_component_bind_groups::<T>,
            RenderStage::PrepareBindGroups,
        );
        app.add_system(
            remove_component_bind_groups::<T>,
            RenderStage::PrepareBindGroups,
        );
        Ok(())
    }
}

fn prepare_component_bind_groups<T: Component + AsBindGroup>(
    commands: Commands,
    device: Res<WgpuDevice>,
    layouts: Res<BindGroupLayouts>,
    mut query: Query<(Entity, &T, Option<&BindGroup<T>>)>,
) {
    let mut layout = None;
    for (entity, data, bind_group) in query.iter() {
        // a bind group can only go out of date through `&mut T`, which marks `T` changed
        let outdated = match bind_group {
            None => true,
            Some(bind_group) => data.is_changed() && bind_group.is_outdated(&data),
        };
        if outdated {
            let layout = layout.get_or_insert_with(|| layouts.get::<T>(&device));
            commands.insert_component(entity, BindGroup::new(&device, layout, &*data));
        }
    }
}

fn remove_component_bind_groups<T: Component + AsBindGroup>(
    commands: Commands,
    removed: RemovedComponents<T>,
) {
    for entity in removed {
        commands.remove_component::<BindGroup<T>>(entity);
    }
}

/// Keeps a `BindGroup<T>` resource next to resource `T`: created when `T` appears, rebuilt when
/// the resources `T` binds are replaced, removed when `T` is removed.
pub struct ResourceBindGroupPlugin<T: Component + AsBindGroup>(PhantomData<T>);

impl<T: Component + AsBindGroup> Default for ResourceBindGroupPlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Component + AsBindGroup> Plugin for ResourceBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(
            prepare_resource_bind_group::<T>.run_if(resource_exists::<T>()),
            RenderStage::PrepareBindGroups,
        );
        app.add_system(
            remove_resource_bind_group::<T>
                .run_if(not(resource_exists::<T>()))
                .run_if(resource_exists::<BindGroup<T>>()),
            RenderStage::PrepareBindGroups,
        );
        Ok(())
    }
}

fn prepare_resource_bind_group<T: Component + AsBindGroup>(
    commands: Commands,
    data: Res<T>,
    bind_group: Option<Res<BindGroup<T>>>,
    device: Res<WgpuDevice>,
    layouts: Res<BindGroupLayouts>,
) {
    let outdated = match bind_group {
        None => true,
        Some(bind_group) => data.is_changed() && bind_group.is_outdated(&data),
    };
    if outdated {
        let layout = layouts.get::<T>(&device);
        commands.insert_resource(BindGroup::new(&device, &layout, &*data));
    }
}

fn remove_resource_bind_group<T: Component + AsBindGroup>(commands: Commands) {
    commands.remove_resource::<BindGroup<T>>();
}
