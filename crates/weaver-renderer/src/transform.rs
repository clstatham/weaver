use weaver_app::{plugin::Plugin, App};
use weaver_util::prelude::Result;

use weaver_core::transform::Transform;
use weaver_ecs::{entity::Entity, world::World};

use crate::extract::{RenderComponent, RenderComponentPlugin};

impl RenderComponent for Transform {
    type ExtractQuery<'a> = &'a Transform;

    fn extract_render_component(
        entity: Entity,
        main_world: &mut World,
        _render_world: &mut World,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        main_world
            .get_component::<Transform>(entity)
            .as_deref()
            .copied()
    }

    fn update_render_component(
        &mut self,
        entity: Entity,
        main_world: &mut World,
        _render_world: &mut World,
    ) -> Result<()> {
        let transform = main_world.get_component::<Transform>(entity).unwrap();
        *self = *transform;
        Ok(())
    }
}

pub struct TransformPlugin;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(RenderComponentPlugin::<Transform>::default())?;
        Ok(())
    }
}
