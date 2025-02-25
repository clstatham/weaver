use std::{any::TypeId, ops::Deref, sync::Arc};

use weaver_app::{App, plugin::Plugin};
use weaver_asset::{AssetApp, AssetId, Assets, Handle, UntypedHandle, prelude::Asset};
use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    entity::{Entity, EntityMap},
    prelude::Component,
    query::{Query, With},
};
use weaver_util::prelude::*;

use crate::{RenderStage, WgpuDevice, asset::RenderAsset};

#[derive(Default)]
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

#[derive(Default)]
pub struct ComponentBindGroupStaleness(pub EntityMap<bool>);

impl ComponentBindGroupStaleness {
    pub fn set_stale(&mut self, entity: Entity, stale: bool) {
        self.0.insert(entity, stale);
    }

    pub fn is_stale(&self, entity: Entity) -> bool {
        *self.0.get(&entity).unwrap_or(&false)
    }
}

#[derive(Default)]
pub struct ResourceBindGroupStaleness(pub TypeIdMap<bool>);

impl ResourceBindGroupStaleness {
    pub fn set_stale<T: Component>(&mut self, stale: bool) {
        self.0.insert(TypeId::of::<T>(), stale);
    }

    pub fn is_stale<T: Component>(&self) -> bool {
        *self.0.get(&TypeId::of::<T>()).unwrap_or(&false)
    }
}

#[derive(Default)]
pub struct AssetBindGroupStaleness(pub FxHashMap<AssetId, bool>);

impl AssetBindGroupStaleness {
    pub fn set_stale(&mut self, asset_id: AssetId, stale: bool) {
        self.0.insert(asset_id, stale);
    }

    pub fn is_stale(&self, asset_id: AssetId) -> bool {
        *self.0.get(&asset_id).unwrap_or(&false)
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
}

#[derive(Asset)]
pub struct BindGroup<T: CreateBindGroup> {
    bind_group: Arc<wgpu::BindGroup>,
    _marker: std::marker::PhantomData<T>,
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
        app.add_system(
            create_component_bind_group::<T>,
            RenderStage::ExtractBindGroup,
        );
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_component_bind_group<T: Component + CreateBindGroup>(
    commands: Commands,
    device: Res<WgpuDevice>,
    mut layout_cache: ResMut<BindGroupLayoutCache>,
    mut item_query: Query<(Entity, &T)>,
    mut bind_group_query: Query<&BindGroup<T>>,
    mut staleness: ResMut<ComponentBindGroupStaleness>,
) {
    let mut to_add = Vec::new();
    let mut to_remove = Vec::new();
    for (entity, item) in item_query.iter() {
        let mut stale = false;
        if staleness.is_stale(entity) {
            // commands.remove_component::<BindGroup<T>>(entity).await;
            to_remove.push(entity);
            stale = true;
        }
        if stale || bind_group_query.get(entity).is_none() {
            let bind_group = BindGroup::new(&device, &*item, &mut layout_cache);
            staleness.set_stale(entity, false);
            // commands.insert_component(entity, bind_group).await;
            to_add.push((entity, bind_group));
        }
    }

    drop((item_query, bind_group_query));

    for (entity, bind_group) in to_add {
        commands.insert_component(entity, bind_group);
    }

    for entity in to_remove {
        commands.remove_component::<BindGroup<T>>(entity);
    }
}

pub struct ResourceBindGroupPlugin<T: Component + CreateBindGroup>(std::marker::PhantomData<T>);

impl<T: Component + CreateBindGroup> Default for ResourceBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: Component + CreateBindGroup> Plugin for ResourceBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(
            create_resource_bind_group::<T>,
            RenderStage::ExtractBindGroup,
        );
        Ok(())
    }
}

async fn create_resource_bind_group<T: Component + CreateBindGroup>(
    commands: Commands,
    data: Res<T>,
    bind_group: Option<Res<BindGroup<T>>>,
    device: Res<WgpuDevice>,
    mut layout_cache: ResMut<BindGroupLayoutCache>,
    mut staleness: ResMut<ResourceBindGroupStaleness>,
) {
    let mut stale = bind_group.is_none();
    if staleness.is_stale::<T>() {
        drop(bind_group);
        commands.remove_resource::<BindGroup<T>>();
        stale = true;
    }
    if stale {
        let bind_group = BindGroup::new(&device, &*data, &mut layout_cache);
        staleness.set_stale::<T>(false);
        commands.insert_resource(bind_group);
    }
}

#[derive(Default)]
pub struct ExtractedAssetBindGroups {
    bind_groups: Lock<FxHashMap<UntypedHandle, UntypedHandle>>,
}

impl ExtractedAssetBindGroups {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, handle: UntypedHandle, bind_group: UntypedHandle) {
        self.bind_groups.write().insert(handle, bind_group);
    }

    pub fn contains(&self, handle: &UntypedHandle) -> bool {
        self.bind_groups.read().contains_key(handle)
    }

    pub fn read(&self) -> Read<FxHashMap<UntypedHandle, UntypedHandle>> {
        self.bind_groups.read()
    }

    pub fn write(&self) -> Write<FxHashMap<UntypedHandle, UntypedHandle>> {
        self.bind_groups.write()
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
        app.add_system(create_asset_bind_group::<T>, RenderStage::ExtractBindGroup);
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
async fn create_asset_bind_group<T: CreateBindGroup + RenderAsset>(
    commands: Commands,
    device: Res<WgpuDevice>,
    mut assets: ResMut<Assets<T>>,
    mut bind_group_assets: ResMut<Assets<BindGroup<T>>>,
    mut query: Query<(Entity, &Handle<T>)>,
    asset_bind_groups: Res<ExtractedAssetBindGroups>,
    mut layout_cache: ResMut<BindGroupLayoutCache>,
    mut bind_group_handle_query: Query<(Entity, With<Handle<BindGroup<T>>>)>,
    mut staleness: ResMut<AssetBindGroupStaleness>,
) {
    let mut to_add = Vec::new();
    let mut to_remove = Vec::new();
    for (entity, handle) in query.iter() {
        // check for bind group staleness
        let mut stale = false;
        if staleness.is_stale(handle.id()) {
            to_remove.push(entity);
            // commands.remove_component::<Handle<BindGroup<T>>>(entity);
            asset_bind_groups
                .bind_groups
                .write()
                .remove(&handle.into_untyped());
            stale = true;
        }

        if bind_group_handle_query.get(entity).is_some() && !stale {
            continue;
        }

        if let Some(bind_group_handle) = asset_bind_groups
            .bind_groups
            .read()
            .get(&handle.into_untyped())
        {
            let bind_group_handle = Handle::<BindGroup<T>>::try_from(*bind_group_handle).unwrap();
            to_add.push((entity, bind_group_handle));
            // commands.insert_component(entity, bind_group_handle);
        } else {
            let asset = assets.get(*handle).unwrap();
            staleness.set_stale(handle.id(), false);
            let bind_group = BindGroup::new(&device, &*asset, &mut layout_cache);
            log::trace!("Created bind group for asset: {:?}", T::type_name());
            let bind_group_handle = bind_group_assets.insert(bind_group);
            asset_bind_groups.insert(handle.into_untyped(), bind_group_handle.into_untyped());
            // commands.insert_component(entity, bind_group_handle);
            to_add.push((entity, bind_group_handle));
        }
    }

    drop((query, bind_group_handle_query));

    for entity in to_remove {
        commands.remove_component::<Handle<BindGroup<T>>>(entity);
    }

    for (entity, bind_group_handle) in to_add {
        commands.insert_component(entity, bind_group_handle);
    }
}
