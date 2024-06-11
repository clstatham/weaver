use std::{any::type_name, rc::Rc};

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    component::Component, entity::Entity, query::QueryFilter, system::SystemStage, world::World,
};

use crate::Renderer;

pub trait RenderComponent: Component {
    type ExtractQuery<'a>: QueryFilter + 'a;
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

fn extract_render_components<T: RenderComponent>(world: Rc<World>) -> anyhow::Result<()> {
    let query = world.clone().query::<T::ExtractQuery<'_>>();
    let renderer = world
        .get_resource::<Renderer>()
        .expect("Renderer resource not present before extracting render components");

    for entity in query.entity_iter() {
        if !world.has_component::<T>(entity) {
            if let Some(component) = T::extract_render_component(entity, &world, &renderer) {
                log::debug!("Extracted render component: {:?}", type_name::<T>());
                world.insert_component(entity, component);
            }
        }
    }

    Ok(())
}

fn update_render_components<T: RenderComponent>(world: Rc<World>) -> anyhow::Result<()> {
    let query = world.clone().query::<T::ExtractQuery<'_>>();
    let renderer = world
        .get_resource::<Renderer>()
        .expect("Renderer resource not present before updating render components");

    for entity in query.entity_iter() {
        if let Some(mut component) = world.get_component_mut::<T>(entity) {
            component.update_render_component(entity, &world, &renderer)?;
        }
    }

    Ok(())
}

pub trait RenderResource: Component {
    fn extract_render_resource(world: Rc<World>, renderer: &Renderer) -> Option<Self>
    where
        Self: Sized;
    fn update_render_resource(
        &mut self,
        world: Rc<World>,
        renderer: &Renderer,
    ) -> anyhow::Result<()>;
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
        app.add_system(update_render_resource::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn extract_render_resource<T: RenderResource>(world: Rc<World>) -> anyhow::Result<()> {
    if !world.has_resource::<T>() {
        let renderer = world
            .get_resource::<Renderer>()
            .expect("Renderer resource not present before extracting render resource");

        if let Some(component) = T::extract_render_resource(world.clone(), &renderer) {
            log::debug!("Extracted render resource: {:?}", type_name::<T>());
            drop(renderer);
            world.insert_resource(component);
        }
    }

    Ok(())
}

fn update_render_resource<T: RenderResource>(world: Rc<World>) -> anyhow::Result<()> {
    let renderer = world
        .get_resource::<Renderer>()
        .expect("Renderer resource not present before updating render resource");

    if let Some(mut resource) = world.get_resource_mut::<T>() {
        resource.update_render_resource(world, &renderer)?;
    }

    Ok(())
}
