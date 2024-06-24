use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    component::{Component, Resource},
    entity::Entity,
    query::QueryFetch,
    world::{World, WorldLock, WriteWorld},
};
use weaver_util::prelude::Result;

use crate::{Extract, ExtractBindGroups, ExtractPipelines, MainWorld, ScratchMainWorld};

pub trait RenderComponent: Component {
    type ExtractQuery<'a>: QueryFetch + 'a;
    fn extract_render_component(
        entity: Entity,
        main_world: &mut World,
        render_world: &mut World,
    ) -> Option<Self>
    where
        Self: Sized;
    fn update_render_component(
        &mut self,
        entity: Entity,
        main_world: &mut World,
        render_world: &mut World,
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

pub fn extract_render_component<T: RenderComponent>(mut render_world: WriteWorld) -> Result<()> {
    let main_world = render_world.get_resource::<MainWorld>().unwrap();
    let query = main_world.query::<T::ExtractQuery<'_>>();

    for (entity, extract_query) in query.iter() {
        if !render_world.has_component::<T>(entity) {
            if let Some(component) =
                T::extract_render_component(entity, &mut main_world.write(), &mut render_world)
            {
                log::debug!(
                    "Extracted render component: {:?}",
                    std::any::type_name::<T>()
                );
                drop(extract_query);
                render_world.insert_component(entity, component);
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

pub fn update_render_component<T: RenderComponent>(mut render_world: WriteWorld) -> Result<()> {
    let main_world = render_world.get_resource::<MainWorld>().unwrap();
    let query = main_world.query::<T::ExtractQuery<'_>>();

    for (entity, _) in query.iter_changed() {
        if let Some(mut component) = render_world.get_component_mut::<T>(entity) {
            component.update_render_component(
                entity,
                &mut main_world.write(),
                &mut render_world,
            )?;
        }
    }

    Ok(())
}

pub trait RenderResource: Resource {
    type UpdateQuery: QueryFetch + 'static;

    fn extract_render_resource(main_world: &mut World, render_world: &mut World) -> Option<Self>
    where
        Self: Sized;
    fn update_render_resource(
        &mut self,
        main_world: &mut World,
        render_world: &mut World,
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

pub fn extract_render_resource<T: RenderResource>(mut render_world: WriteWorld) -> Result<()> {
    if !render_world.has_resource::<T>() {
        let main_world = render_world.get_resource::<MainWorld>().unwrap();

        if let Some(resource) =
            T::extract_render_resource(&mut main_world.write(), &mut render_world)
        {
            log::debug!(
                "Extracted render resource: {:?}",
                std::any::type_name::<T>()
            );
            drop(main_world);
            render_world.insert_resource(resource);
        } else {
            log::warn!(
                "Failed to extract render resource: {:?}",
                std::any::type_name::<T>()
            );
        }
    }

    Ok(())
}

pub fn update_render_resource<T: RenderResource>(mut render_world: WriteWorld) -> Result<()> {
    if let Some(mut resource) = render_world.get_resource_mut::<T>() {
        // let query = render_world.query::<T::UpdateQuery>();
        // if query.iter_changed().next().is_none() {
        //     return Ok(());
        // }
        let main_world = render_world.get_resource::<MainWorld>().unwrap();
        resource.update_render_resource(&mut main_world.write(), &mut render_world)?;
    }

    Ok(())
}

pub fn render_extract(main_world: &mut WorldLock, render_world: &mut WorldLock) -> Result<()> {
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
