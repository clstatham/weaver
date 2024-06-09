use std::{ops::Deref, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_asset::{Assets, Handle};
use weaver_ecs::{query::Query, system::SystemStage, world::World};

use crate::Renderer;

pub trait CreateBindGroup: 'static {
    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized;
    fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup;
}

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

fn create_bind_groups<T: CreateBindGroup>(world: &World) -> anyhow::Result<()> {
    let renderer = world.get_resource::<Renderer>().unwrap();
    let device = renderer.device();

    let query = world.query(&Query::new().read::<T>());

    for entity in query.iter() {
        if !world.has_component::<BindGroup<T>>(entity) {
            let data = world.get_component::<T>(entity).unwrap();
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

fn create_resource_bind_group<T: CreateBindGroup>(world: &World) -> anyhow::Result<()> {
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

pub struct AssetBindGroupPlugin<T: CreateBindGroup>(std::marker::PhantomData<T>);

impl<T: CreateBindGroup> Default for AssetBindGroupPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: CreateBindGroup> Plugin for AssetBindGroupPlugin<T> {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(create_asset_bind_group::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn create_asset_bind_group<T: CreateBindGroup>(world: &World) -> anyhow::Result<()> {
    let renderer = world.get_resource::<Renderer>().unwrap();
    let device = renderer.device();

    let assets = world.get_resource::<Assets>().unwrap();

    let query = world.query(&Query::new().read::<Handle<T>>());

    for entity in query.iter() {
        if !world.has_component::<BindGroup<T>>(entity) {
            let handle = query.get::<Handle<T>>(entity).unwrap();
            let data = assets.get::<T>(*handle).unwrap();
            let bind_group = BindGroup::new(device, data);
            drop(handle);
            world.insert_component(entity, bind_group);
        }
    }

    Ok(())
}
