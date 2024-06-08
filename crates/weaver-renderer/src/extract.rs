use std::any::type_name;

use weaver_app::{plugin::Plugin, prelude::App};
use weaver_ecs::{
    component::Component, entity::Entity, query::Query, system::SystemStage, world::World,
};

pub trait RenderComponent: Component {
    fn query() -> Query;
    fn extract_render_component(entity: Entity, world: &World) -> Option<Self>
    where
        Self: Sized;
}

pub struct ExtractRenderComponentPlugin<T: RenderComponent>(std::marker::PhantomData<T>);

impl<T: RenderComponent> Default for ExtractRenderComponentPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: RenderComponent> Plugin for ExtractRenderComponentPlugin<T> {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.add_system(extract_render_components::<T>, SystemStage::PreRender)?;
        Ok(())
    }
}

fn extract_render_components<T: RenderComponent>(world: &World) -> anyhow::Result<()> {
    let query = world.query(&T::query());

    for entity in query.iter() {
        if !world.has_component::<T>(entity) {
            if let Some(component) = T::extract_render_component(entity, world) {
                log::info!("Extracted render component: {:?}", type_name::<T>());
                world.insert_component(entity, component);
            }
        }
    }

    Ok(())
}
