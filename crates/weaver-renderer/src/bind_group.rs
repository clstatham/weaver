use std::{collections::HashMap, ops::Deref, rc::Rc, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_asset::{prelude::Asset, Assets, Handle, UntypedHandle};
use weaver_ecs::{prelude::Component, system::SystemStage, world::World};

use crate::{asset::RenderAsset, Renderer};

pub trait CreateBindGroup: Component {
    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized;
    fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup;
}

#[derive(Component, Asset)]
pub struct BindGroup<T: CreateBindGroup> {
    bind_group: Arc<wgpu::BindGroup>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: CreateBindGroup> BindGroup<T> {
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

impl<T> Deref for BindGroup<T>
where
    T: CreateBindGroup,
{
    type Target = wgpu::BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.bind_group
    }
}

pub struct ComponentBindGroupPlugin<T: CreateBindGroup>(std::marker::PhantomData<T>);

impl<T: CreateBindGroup> Default for ComponentBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: CreateBindGroup> Plugin for ComponentBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(create_bind_groups::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn create_bind_groups<T: CreateBindGroup>(world: Rc<World>) -> anyhow::Result<()> {
    let renderer = world.clone().get_resource::<Renderer>().unwrap();
    let device = renderer.device();

    let query = world.query::<&T>();

    for (entity, data) in query.iter() {
        if !world.has_component::<BindGroup<T>>(entity) {
            let bind_group = BindGroup::new(device, &*data);
            drop(data);
            world.insert_component(entity, bind_group);
        }
    }

    Ok(())
}

pub struct ResourceBindGroupPlugin<T: CreateBindGroup>(std::marker::PhantomData<T>);

impl<T: CreateBindGroup> Default for ResourceBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: CreateBindGroup> Plugin for ResourceBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(create_resource_bind_group::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn create_resource_bind_group<T: CreateBindGroup>(world: Rc<World>) -> anyhow::Result<()> {
    let renderer = world.get_resource::<Renderer>().unwrap();
    let device = renderer.device();

    if !world.has_resource::<BindGroup<T>>() {
        let data = world.get_resource::<T>().unwrap();
        let bind_group = BindGroup::new(device, &*data);
        drop(data);
        drop(renderer);
        world.insert_resource(bind_group);
    }

    Ok(())
}

#[derive(Default, Component)]
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
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(create_asset_bind_group::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn create_asset_bind_group<T: CreateBindGroup + RenderAsset>(
    world: Rc<World>,
) -> anyhow::Result<()> {
    let renderer = world.get_resource::<Renderer>().unwrap();
    let device = renderer.device();

    let mut assets = world.get_resource_mut::<Assets>().unwrap();

    let query = world.query::<&Handle<T>>();

    for (entity, handle) in query.iter() {
        if world.has_component::<Handle<BindGroup<T>>>(entity) {
            continue;
        }

        let mut asset_bind_groups = world
            .get_resource_mut::<ExtractedAssetBindGroups>()
            .unwrap();

        if let Some(bind_group_handle) = asset_bind_groups.bind_groups.get(&handle.into_untyped()) {
            let bind_group_handle = Handle::<BindGroup<T>>::try_from(*bind_group_handle).unwrap();
            drop(handle);
            world.insert_component(entity, bind_group_handle);
        } else {
            let asset = assets.get::<T>(*handle).unwrap();
            let bind_group = BindGroup::new(device, asset);
            let bind_group_handle = assets.insert(bind_group, None);
            asset_bind_groups.insert(handle.into_untyped(), bind_group_handle.into_untyped());
            drop(handle);
            world.insert_component(entity, bind_group_handle);
        }
    }

    Ok(())
}
