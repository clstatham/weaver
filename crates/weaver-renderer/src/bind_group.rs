use std::{any::TypeId, collections::HashMap, ops::Deref, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_asset::{prelude::Asset, Assets, Handle, UntypedHandle};
use weaver_ecs::{
    prelude::{Component, Reflect, Resource},
    world::World,
};
use weaver_util::{
    prelude::{bail, DowncastSync, Result},
    TypeIdMap,
};

use crate::{asset::RenderAsset, ExtractBindGroups, WgpuDevice};

#[derive(Resource, Default)]
pub struct BindGroupLayoutCache {
    cache: TypeIdMap<BindGroupLayout>,
}

impl BindGroupLayoutCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_create<T>(&mut self, device: &wgpu::Device) -> BindGroupLayout
    where
        T: CreateBindGroup,
    {
        if let Some(cached_layout) = self.cache.get(&TypeId::of::<T>()) {
            cached_layout.clone()
        } else {
            let bind_group = T::create_bind_group_layout(device);
            let cached_layout = BindGroupLayout {
                bind_group: Arc::new(bind_group),
            };
            self.cache.insert(TypeId::of::<T>(), cached_layout.clone());
            self.cache.get(&TypeId::of::<T>()).unwrap().clone()
        }
    }
}

#[derive(Clone)]
pub struct BindGroupLayout {
    bind_group: Arc<wgpu::BindGroupLayout>,
}

impl BindGroupLayout {
    pub fn get_or_create<T>(device: &wgpu::Device, cache: &mut BindGroupLayoutCache) -> Self
    where
        T: CreateBindGroup,
    {
        cache.get_or_create::<T>(device)
    }
}

impl Deref for BindGroupLayout {
    type Target = wgpu::BindGroupLayout;

    fn deref(&self) -> &Self::Target {
        &self.bind_group
    }
}

pub trait CreateBindGroup: DowncastSync {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized;
    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &BindGroupLayout,
    ) -> wgpu::BindGroup;

    fn bind_group_stale(&self) -> bool {
        false
    }

    #[allow(unused_variables)]
    fn set_bind_group_stale(&mut self, stale: bool) {}
}

#[derive(Component, Resource, Clone, Reflect)]
pub struct BindGroup<T: CreateBindGroup> {
    #[reflect(ignore)]
    bind_group: Arc<wgpu::BindGroup>,
    #[reflect(ignore)]
    _marker: std::marker::PhantomData<T>,
}

impl<T: CreateBindGroup> Asset for BindGroup<T> {
    fn load(_assets: &mut Assets, _path: &std::path::Path) -> Result<Self> {
        bail!("ComponentBindGroup cannot be loaded from a file")
    }
}

impl<T: CreateBindGroup> BindGroup<T> {
    pub fn new(device: &wgpu::Device, data: &T, cache: &mut BindGroupLayoutCache) -> Self {
        let cached_layout = BindGroupLayout::get_or_create::<T>(device, cache);
        let bind_group = data.create_bind_group(device, &cached_layout);
        Self {
            bind_group: Arc::new(bind_group),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn bind_group(&self) -> &Arc<wgpu::BindGroup> {
        &self.bind_group
    }
}

impl<T> Deref for BindGroup<T>
where
    T: CreateBindGroup,
{
    type Target = wgpu::BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.bind_group
    }
}

pub struct ComponentBindGroupPlugin<T: Component + CreateBindGroup>(std::marker::PhantomData<T>);

impl<T: Component + CreateBindGroup> Default for ComponentBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: Component + CreateBindGroup> Plugin for ComponentBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(create_component_bind_group::<T>, ExtractBindGroups);
        Ok(())
    }
}

fn create_component_bind_group<T: Component + CreateBindGroup>(
    render_world: &mut World,
) -> Result<()> {
    let device = render_world.get_resource::<WgpuDevice>().unwrap();

    let query = render_world.query::<&mut T>();

    for (entity, mut data) in query.iter() {
        if data.bind_group_stale() {
            render_world.remove_component::<BindGroup<T>>(entity);
        }
        if !render_world.has_component::<BindGroup<T>>(entity) {
            let mut layout_cache = render_world
                .get_resource_mut::<BindGroupLayoutCache>()
                .unwrap();
            let bind_group = BindGroup::new(&device, &*data, &mut layout_cache);
            data.set_bind_group_stale(false);
            drop(data);
            render_world.insert_component(entity, bind_group);
        }
    }

    Ok(())
}

pub struct ResourceBindGroupPlugin<T: Resource + CreateBindGroup>(std::marker::PhantomData<T>);

impl<T: Resource + CreateBindGroup> Default for ResourceBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: Resource + CreateBindGroup> Plugin for ResourceBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(create_resource_bind_group::<T>, ExtractBindGroups);
        Ok(())
    }
}

fn create_resource_bind_group<T: Resource + CreateBindGroup>(
    render_world: &mut World,
) -> Result<()> {
    let Some(mut data) = render_world.get_resource_mut::<T>() else {
        return Ok(());
    };
    if data.bind_group_stale() {
        render_world.remove_resource::<BindGroup<T>>();
    }
    if !render_world.has_resource::<BindGroup<T>>() {
        let device = render_world.get_resource::<WgpuDevice>().unwrap();
        let mut layout_cache = render_world
            .get_resource_mut::<BindGroupLayoutCache>()
            .unwrap();
        let bind_group = BindGroup::new(&device, &*data, &mut layout_cache);
        data.set_bind_group_stale(false);
        drop(data);
        drop(device);
        drop(layout_cache);
        render_world.insert_resource(bind_group);
    }

    Ok(())
}

#[derive(Default, Resource)]
pub struct ExtractedAssetBindGroups {
    bind_groups: HashMap<UntypedHandle, UntypedHandle>,
}

impl ExtractedAssetBindGroups {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, handle: UntypedHandle, bind_group: UntypedHandle) {
        self.bind_groups.insert(handle, bind_group);
    }

    pub fn contains(&self, handle: &UntypedHandle) -> bool {
        self.bind_groups.contains_key(handle)
    }
}

pub struct AssetBindGroupPlugin<T: CreateBindGroup + RenderAsset>(std::marker::PhantomData<T>);

impl<T: CreateBindGroup + RenderAsset> Default for AssetBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: CreateBindGroup + RenderAsset> Plugin for AssetBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(create_asset_bind_group::<T>, ExtractBindGroups);
        Ok(())
    }
}

fn create_asset_bind_group<T: CreateBindGroup + RenderAsset>(
    render_world: &mut World,
) -> Result<()> {
    let device = render_world.get_resource::<WgpuDevice>().unwrap();
    let mut assets = render_world.get_resource_mut::<Assets>().unwrap();

    let query = render_world.query::<&Handle<T>>();

    for (entity, handle) in query.iter() {
        let mut asset_bind_groups = render_world
            .get_resource_mut::<ExtractedAssetBindGroups>()
            .unwrap();

        // check for bind group staleness
        if let Some(asset) = assets.get(*handle) {
            if asset.bind_group_stale() {
                render_world.remove_component::<Handle<BindGroup<T>>>(entity);
                asset_bind_groups.bind_groups.remove(&handle.into_untyped());
            }
        }

        if render_world.has_component::<Handle<BindGroup<T>>>(entity) {
            continue;
        }

        if let Some(bind_group_handle) = asset_bind_groups.bind_groups.get(&handle.into_untyped()) {
            let bind_group_handle = Handle::<BindGroup<T>>::try_from(*bind_group_handle).unwrap();
            drop(handle);
            render_world.insert_component(entity, bind_group_handle);
        } else {
            let mut asset = assets.get_mut::<T>(*handle).unwrap();
            let mut layout_cache = render_world
                .get_resource_mut::<BindGroupLayoutCache>()
                .unwrap();
            asset.set_bind_group_stale(false);
            let bind_group = BindGroup::new(&device, &*asset, &mut layout_cache);
            let bind_group_handle = assets.insert(bind_group);
            asset_bind_groups.insert(handle.into_untyped(), bind_group_handle.into_untyped());
            drop(handle);
            render_world.insert_component(entity, bind_group_handle);
        }
    }

    Ok(())
}
