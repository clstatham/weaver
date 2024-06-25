use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    commands::Commands,
    component::{Component, Res, ResMut, Resource},
    entity::Entity,
    query::{Query, QueryFetch},
    world::World,
};
use weaver_util::prelude::Result;

use crate::{
    Extract, ExtractBindGroups, ExtractPipelines, MainWorld, ScratchMainWorld, WgpuDevice,
    WgpuQueue,
};

pub trait RenderComponent: Component {
    type ExtractQuery<'a>: QueryFetch + 'a;
    fn extract_render_component(
        entity: Entity,
        main_world: &mut World,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<Self>
    where
        Self: Sized;
    fn update_render_component(
        &mut self,
        entity: Entity,
        main_world: &mut World,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()>;
}

pub struct RenderComponentPlugin<T: RenderComponent>(std::marker::PhantomData<T>);

impl<T: RenderComponent> Default for RenderComponentPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: RenderComponent> Plugin for RenderComponentPlugin<T> {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_system(extract_render_component::<T>, Extract);
        render_app.add_system_after(
            update_render_component::<T>,
            extract_render_component::<T>,
            Extract,
        );
        Ok(())
    }
}

pub fn extract_render_component<T: RenderComponent>(
    commands: Commands,
    mut main_world: ResMut<MainWorld>,
    render_query: Query<&T>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    let query = main_world.query::<T::ExtractQuery<'_>>();
    let entities = query.entity_iter(&main_world).collect::<Vec<_>>();
    for entity in entities {
        if render_query.get(entity).is_none() {
            if let Some(component) =
                T::extract_render_component(entity, &mut main_world, &device, &queue)
            {
                log::debug!(
                    "Extracted render component: {:?}",
                    std::any::type_name::<T>()
                );
                commands.insert_component(entity, component);
            } else {
                log::warn!(
                    "Failed to extract render component: {:?}",
                    std::any::type_name::<T>()
                );
            }
        }
    }

    Ok(())
}

pub fn update_render_component<T: RenderComponent>(
    mut main_world: ResMut<MainWorld>,
    render_query: Query<&mut T>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    let query = main_world.query::<T::ExtractQuery<'_>>();

    let entities = query.entity_iter(&main_world).collect::<Vec<_>>();

    for entity in entities {
        if let Some(mut component) = render_query.get(entity) {
            component.update_render_component(entity, &mut main_world, &device, &queue)?;
        }
    }

    Ok(())
}

pub trait RenderResource: Resource {
    fn extract_render_resource(
        main_world: &mut World,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<Self>
    where
        Self: Sized;
    fn update_render_resource(
        &mut self,
        main_world: &mut World,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()>;
}

pub struct RenderResourcePlugin<T: RenderResource>(std::marker::PhantomData<T>);

impl<T: RenderResource> Default for RenderResourcePlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: RenderResource> Plugin for RenderResourcePlugin<T> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(extract_render_resource::<T>, Extract);
        app.add_system_after(
            update_render_resource::<T>,
            extract_render_resource::<T>,
            Extract,
        );
        Ok(())
    }
}

pub struct RenderResourceDependencyPlugin<T: RenderResource, Dep: RenderResource>(
    std::marker::PhantomData<(T, Dep)>,
);

impl<T: RenderResource, Dep: RenderResource> Default for RenderResourceDependencyPlugin<T, Dep> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: RenderResource, Dep: RenderResource> Plugin for RenderResourceDependencyPlugin<T, Dep> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system_after(
            extract_render_resource::<T>,
            extract_render_resource::<Dep>,
            Extract,
        );
        app.add_system_after(
            update_render_resource::<T>,
            extract_render_resource::<Dep>,
            Extract,
        );
        Ok(())
    }
}

pub fn extract_render_resource<T: RenderResource>(
    commands: Commands,
    mut main_world: ResMut<MainWorld>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
    resource: Option<Res<T>>,
) -> Result<()> {
    if resource.is_none() {
        if let Some(resource) = T::extract_render_resource(&mut main_world, &device, &queue) {
            log::debug!(
                "Extracted render resource: {:?}",
                std::any::type_name::<T>()
            );
            drop(main_world);
            commands.insert_resource(resource);
        } else {
            log::warn!(
                "Failed to extract render resource: {:?}",
                std::any::type_name::<T>()
            );
        }
    }

    Ok(())
}

pub fn update_render_resource<T: RenderResource>(
    mut resource: ResMut<T>,
    mut main_world: ResMut<MainWorld>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    resource.update_render_resource(&mut main_world, &device, &queue)?;

    Ok(())
}

pub fn render_extract(main_world: &mut World, render_world: &mut World) -> Result<()> {
    let scratch_world = main_world.remove_resource::<ScratchMainWorld>().unwrap();
    let inserted_world = std::mem::replace(main_world, scratch_world.0);
    render_world.insert_resource(MainWorld(inserted_world));

    render_world.run_stage::<Extract>()?;
    render_world.run_stage::<ExtractBindGroups>()?;
    render_world.run_stage::<ExtractPipelines>()?;

    let inserted_world = render_world.remove_resource::<MainWorld>().unwrap();
    let scratch_world = std::mem::replace(main_world, inserted_world.0);
    main_world.insert_resource(ScratchMainWorld(scratch_world));

    Ok(())
}
