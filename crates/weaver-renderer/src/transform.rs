use weaver_app::{plugin::Plugin, App};
use weaver_ecs::query::QueryableItem;
use weaver_util::Result;

use weaver_core::transform::Transform;

use crate::extract::{ExtractComponent, ExtractComponentPlugin};

impl ExtractComponent for Transform {
    type ExtractQueryFetch = &'static Transform;
    type Out = Transform;

    fn extract_render_component(
        item: QueryableItem<'_, Self::ExtractQueryFetch>,
    ) -> Option<Self::Out> {
        Some(*item)
    }
}

pub struct TransformPlugin;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(ExtractComponentPlugin::<Transform>::default())?;
        Ok(())
    }
}
