use std::{any::TypeId, collections::HashMap, ops::Deref, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_asset::{prelude::Asset, AddAsset, Assets, Handle, UntypedHandle};
use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    prelude::{Component, Reflect, Resource},
    query::Query,
};
use weaver_util::{
    prelude::{DowncastSync, Result},
    TypeIdMap,
};

use crate::{asset::RenderAsset, ExtractBindGroupStage, WgpuDevice};

#[derive(Resource, Default)]
pub struct BindGroupLayoutCache {
    cache: TypeIdMap<BindGroupLayout>,
}

impl BindGroupLayoutCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: DowncastSync>(&mut self, layout: BindGroupLayout) {
        self.cache.insert(TypeId::of::<T>(), layout);
    }

    pub fn get<T: DowncastSync>(&self) -> Option<&BindGroupLayout> {
        self.cache.get(&TypeId::of::<T>())
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

    pub fn from_raw(bind_group: wgpu::BindGroupLayout) -> Self {
        Self {
            bind_group: Arc::new(bind_group),
        }
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

#[derive(Component, Resource, Reflect)]
pub struct BindGroup<T: CreateBindGroup> {
    #[reflect(ignore)]
    bind_group: Arc<wgpu::BindGroup>,
    #[reflect(ignore)]
    _marker: std::marker::PhantomData<T>,
}

impl<T: Asset + CreateBindGroup> Asset for BindGroup<T> {}

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
        app.add_system(create_component_bind_group::<T>, ExtractBindGroupStage);
        Ok(())
    }
}

fn create_component_bind_group<T: Component + CreateBindGroup>(
    commands: Commands,
    device: Res<WgpuDevice>,
    mut layout_cache: ResMut<BindGroupLayoutCache>,
    item_query: Query<&mut T>,
    bind_group_query: Query<&BindGroup<T>>,
) -> Result<()> {
    for (entity, mut item) in item_query.iter() {
        let mut stale = false;
        if item.bind_group_stale() {
            commands.remove_component::<BindGroup<T>>(entity);
            stale = true;
        }
        if stale || bind_group_query.get(entity).is_none() {
            let bind_group = BindGroup::new(&device, &*item, &mut layout_cache);
            item.set_bind_group_stale(false);
            drop(item);
            commands.insert_component(entity, bind_group);
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
        app.add_system(create_resource_bind_group::<T>, ExtractBindGroupStage);
        Ok(())
    }
}

fn create_resource_bind_group<T: Resource + CreateBindGroup>(
    commands: Commands,
    mut data: ResMut<T>,
    bind_group: Option<Res<BindGroup<T>>>,
    device: Res<WgpuDevice>,
    mut layout_cache: ResMut<BindGroupLayoutCache>,
) -> Result<()> {
    let mut stale = false;
    if data.bind_group_stale() {
        commands.remove_resource::<BindGroup<T>>();
        stale = true;
    }
    if stale || bind_group.is_none() {
        let bind_group = BindGroup::new(&device, &*data, &mut layout_cache);
        data.set_bind_group_stale(false);
        commands.insert_resource(bind_group);
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
        app.add_asset::<BindGroup<T>>();
        app.add_system(create_asset_bind_group::<T>, ExtractBindGroupStage);
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn create_asset_bind_group<T: CreateBindGroup + RenderAsset>(
    commands: Commands,
    device: Res<WgpuDevice>,
    assets: Res<Assets<T>>,
    mut bind_group_assets: ResMut<Assets<BindGroup<T>>>,
    query: Query<&Handle<T>>,
    mut asset_bind_groups: ResMut<ExtractedAssetBindGroups>,
    mut layout_cache: ResMut<BindGroupLayoutCache>,
    bind_group_handle_query: Query<&Handle<BindGroup<T>>>,
) -> Result<()> {
    for (entity, handle) in query.iter() {
        // check for bind group staleness
        let mut stale = false;
        if let Some(asset) = assets.get(*handle) {
            if asset.bind_group_stale() {
                commands.remove_component::<Handle<BindGroup<T>>>(entity);
                asset_bind_groups.bind_groups.remove(&handle.into_untyped());
                stale = true;
            }
        }

        if bind_group_handle_query.get(entity).is_some() && !stale {
            continue;
        }

        if let Some(bind_group_handle) = asset_bind_groups.bind_groups.get(&handle.into_untyped()) {
            let bind_group_handle = Handle::<BindGroup<T>>::try_from(*bind_group_handle).unwrap();
            drop(handle);
            commands.insert_component(entity, bind_group_handle);
        } else {
            let mut asset = assets.get_mut(*handle).unwrap();
            asset.set_bind_group_stale(false);
            let bind_group = BindGroup::new(&device, &*asset, &mut layout_cache);
            log::trace!(
                "Created bind group for asset: {:?}",
                std::any::type_name::<T>()
            );
            let bind_group_handle = bind_group_assets.insert(bind_group);
            asset_bind_groups.insert(handle.into_untyped(), bind_group_handle.into_untyped());
            drop(handle);
            commands.insert_component(entity, bind_group_handle);
        }
    }

    Ok(())
}
