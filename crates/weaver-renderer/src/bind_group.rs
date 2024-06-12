use std::{collections::HashMap, ops::Deref, sync::Arc};

use anyhow::bail;
use weaver_app::{plugin::Plugin, system::SystemStage, App};
use weaver_asset::{prelude::Asset, Assets, Handle, UntypedHandle};
use weaver_ecs::{
    prelude::{Component, Resource},
    world::World,
};

use crate::{asset::RenderAsset, Renderer};

pub trait CreateComponentBindGroup: Component {
    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized;
    fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup;
}

pub trait CreateResourceBindGroup: Resource {
    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized;
    fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup;
}

#[derive(Component, Clone)]
pub struct ComponentBindGroup<T: CreateComponentBindGroup> {
    bind_group: Arc<wgpu::BindGroup>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: CreateComponentBindGroup> Asset for ComponentBindGroup<T> {
    fn load(_assets: &mut Assets, _path: &std::path::Path) -> anyhow::Result<Self> {
        bail!("ComponentBindGroup cannot be loaded from a file")
    }
}

impl<T: CreateComponentBindGroup> ComponentBindGroup<T> {
    pub fn new(device: &wgpu::Device, data: &T) -> Self {
        let bind_group = data.create_bind_group(device);
        Self {
            bind_group: Arc::new(bind_group),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn bind_group(&self) -> &Arc<wgpu::BindGroup> {
        &self.bind_group
    }
}

impl<T> Deref for ComponentBindGroup<T>
where
    T: CreateComponentBindGroup,
{
    type Target = wgpu::BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.bind_group
    }
}

pub struct ComponentBindGroupPlugin<T: CreateComponentBindGroup>(std::marker::PhantomData<T>);

impl<T: CreateComponentBindGroup> Default for ComponentBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: CreateComponentBindGroup> Plugin for ComponentBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(create_bind_groups::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn create_bind_groups<T: CreateComponentBindGroup>(world: &Arc<World>) -> anyhow::Result<()> {
    let renderer = world.clone().get_resource::<Renderer>().unwrap();
    let device = renderer.device();

    let query = world.query::<&T>();

    for (entity, data) in query.iter() {
        if !world.has_component::<ComponentBindGroup<T>>(entity) {
            let bind_group = ComponentBindGroup::new(device, &*data);
            drop(data);
            world.insert_component(entity, bind_group);
        }
    }

    Ok(())
}

#[derive(Resource, Clone)]
pub struct ResourceBindGroup<T: CreateResourceBindGroup> {
    bind_group: Arc<wgpu::BindGroup>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: CreateResourceBindGroup> ResourceBindGroup<T> {
    pub fn new(device: &wgpu::Device, data: &T) -> Self {
        let bind_group = data.create_bind_group(device);
        Self {
            bind_group: Arc::new(bind_group),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn bind_group(&self) -> &Arc<wgpu::BindGroup> {
        &self.bind_group
    }
}

impl<T> Deref for ResourceBindGroup<T>
where
    T: CreateResourceBindGroup,
{
    type Target = wgpu::BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.bind_group
    }
}

pub struct ResourceBindGroupPlugin<T: CreateResourceBindGroup>(std::marker::PhantomData<T>);

impl<T: CreateResourceBindGroup> Default for ResourceBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: CreateResourceBindGroup> Plugin for ResourceBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(create_resource_bind_group::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn create_resource_bind_group<T: CreateResourceBindGroup>(
    world: &Arc<World>,
) -> anyhow::Result<()> {
    let Some(renderer) = world.get_resource::<Renderer>() else {
        return Ok(());
    };
    let device = renderer.device();

    if !world.has_resource::<ResourceBindGroup<T>>() {
        let Some(data) = world.get_resource::<T>() else {
            return Ok(());
        };
        let bind_group = ResourceBindGroup::new(device, &*data);
        drop(data);
        drop(renderer);
        world.insert_resource(bind_group);
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

pub struct AssetBindGroupPlugin<T: CreateComponentBindGroup + RenderAsset>(
    std::marker::PhantomData<T>,
);

impl<T: CreateComponentBindGroup + RenderAsset> Default for AssetBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: CreateComponentBindGroup + RenderAsset> Plugin for AssetBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(create_asset_bind_group::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn create_asset_bind_group<T: CreateComponentBindGroup + RenderAsset>(
    world: &Arc<World>,
) -> anyhow::Result<()> {
    let renderer = world.get_resource::<Renderer>().unwrap();
    let device = renderer.device();

    let mut assets = world.get_resource_mut::<Assets>().unwrap();

    let query = world.query::<&Handle<T>>();

    for (entity, handle) in query.iter() {
        if world.has_component::<Handle<ComponentBindGroup<T>>>(entity) {
            continue;
        }

        let mut asset_bind_groups = world
            .get_resource_mut::<ExtractedAssetBindGroups>()
            .unwrap();

        if let Some(bind_group_handle) = asset_bind_groups.bind_groups.get(&handle.into_untyped()) {
            let bind_group_handle =
                Handle::<ComponentBindGroup<T>>::try_from(*bind_group_handle).unwrap();
            drop(handle);
            world.insert_component(entity, bind_group_handle);
        } else {
            let asset = assets.get::<T>(*handle).unwrap();
            let bind_group = ComponentBindGroup::new(device, asset);
            let bind_group_handle = assets.insert(bind_group);
            asset_bind_groups.insert(handle.into_untyped(), bind_group_handle.into_untyped());
            drop(handle);
            world.insert_component(entity, bind_group_handle);
        }
    }

    Ok(())
}
