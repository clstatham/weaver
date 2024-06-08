use std::any::type_name;

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    component::Component, entity::Entity, query::Query, system::SystemStage, world::World,
};

use crate::Renderer;

pub trait RenderComponent: Component {
    fn extract_query() -> Query;
    fn extract_render_component(entity: Entity, world: &World, renderer: &Renderer) -> Option<Self>
    where
        Self: Sized;
    fn update_render_component(
        &mut self,
        entity: Entity,
        world: &World,
        renderer: &Renderer,
    ) -> anyhow::Result<()>;
}

pub struct RenderComponentPlugin<T: RenderComponent>(std::marker::PhantomData<T>);

impl<T: RenderComponent> Default for RenderComponentPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: RenderComponent> Plugin for RenderComponentPlugin<T> {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(extract_render_components::<T>, SystemStage::PreRender)?;
        app.add_system(update_render_components::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn extract_render_components<T: RenderComponent>(world: &World) -> anyhow::Result<()> {
    let query = world.query(&T::extract_query());
    let renderer = world
        .get_resource::<Renderer>()
        .expect("Renderer resource not present before extracting render components");

    for entity in query.iter() {
        if !world.has_component::<T>(entity) {
            if let Some(component) = T::extract_render_component(entity, world, &renderer) {
                log::debug!("Extracted render component: {:?}", type_name::<T>());
                world.insert_component(entity, component);
            }
        }
    }

    Ok(())
}

fn update_render_components<T: RenderComponent>(world: &World) -> anyhow::Result<()> {
    let query = world.query(&Query::new().write::<T>());
    let renderer = world
        .get_resource::<Renderer>()
        .expect("Renderer resource not present before updating render components");

    for entity in query.iter() {
        if let Some(mut component) = world.get_component_mut::<T>(entity) {
            component.update_render_component(entity, world, &renderer)?;
        }
    }

    Ok(())
}

pub trait RenderResource: Component {
    fn extract_render_resource(world: &World, renderer: &Renderer) -> Option<Self>
    where
        Self: Sized;
    fn update_render_resource(&mut self, world: &World, renderer: &Renderer) -> anyhow::Result<()>;
}

pub struct RenderResourcePlugin<T: RenderResource>(std::marker::PhantomData<T>);

impl<T: RenderResource> Default for RenderResourcePlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: RenderResource> Plugin for RenderResourcePlugin<T> {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(extract_render_resource::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn extract_render_resource<T: RenderResource>(world: &World) -> anyhow::Result<()> {
    if !world.has_resource::<T>() {
        let renderer = world
            .get_resource::<Renderer>()
            .expect("Renderer resource not present before extracting render resource");

        if let Some(component) = T::extract_render_resource(world, &renderer) {
            log::debug!("Extracted render resource: {:?}", type_name::<T>());
            drop(renderer);
            world.insert_resource(component);
        }
    }

    Ok(())
}
