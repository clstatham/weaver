use weaver_app::{plugin::Plugin, App};
use weaver_util::prelude::Result;

use weaver_core::transform::Transform;
use weaver_ecs::prelude::QueryFetchItem;

use crate::extract::{ExtractComponent, ExtractComponentPlugin};

impl ExtractComponent for Transform {
    type ExtractQueryFetch = &'static Transform;
    type ExtractQueryFilter = ();
    type Out = Transform;

    fn extract_render_component(
        item: QueryFetchItem<'_, Self::ExtractQueryFetch>,
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
