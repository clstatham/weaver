use std::any::type_name;

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    component::Component, entity::Entity, query::Query, system::SystemStage, world::World,
};

pub trait RenderComponent: Component {
    fn extract_query() -> Query;
    fn extract_render_component(entity: Entity, world: &World) -> Option<Self>
    where
        Self: Sized;
    fn update_render_component(&mut self, entity: Entity, world: &World) -> anyhow::Result<()>;
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

    for entity in query.iter() {
        if !world.has_component::<T>(entity) {
            if let Some(component) = T::extract_render_component(entity, world) {
                log::debug!("Extracted render component: {:?}", type_name::<T>());
                world.insert_component(entity, component);
            }
        }
    }

    Ok(())
}

fn update_render_components<T: RenderComponent>(world: &World) -> anyhow::Result<()> {
    let query = world.query(&Query::new().write::<T>());

    for entity in query.iter() {
        if let Some(mut component) = world.get_component_mut::<T>(entity) {
            component.update_render_component(entity, world)?;
        }
    }

    Ok(())
}
