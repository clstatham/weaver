//! The color views are cleared to.

use weaver_app::{App, plugin::Plugin};
use weaver_core::color::Color;
use weaver_util::prelude::*;

use crate::{
    RenderApp,
    extract::{ExtractResource, ExtractResourcePlugin},
};

/// The color views are cleared to before drawing. Extracted to the render world when it changes.
#[derive(Clone, Copy)]
pub struct ClearColor(pub Color);

impl ExtractResource for ClearColor {
    type Source = ClearColor;
    fn extract_render_resource(source: &Self::Source) -> Self
    where
        Self: Sized,
    {
        *source
    }
}

/// Inserts a [`ClearColor`] (default black) and extracts it to the render world.
///
/// # Panics
///
/// Panics in `build` if [`RendererPlugin`](crate::RendererPlugin) hasn't been added yet.
#[derive(Clone)]
pub struct ClearColorPlugin(pub Color);

impl Default for ClearColorPlugin {
    fn default() -> Self {
        Self(Color::BLACK)
    }
}

impl Plugin for ClearColorPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(ClearColor(self.0));
        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(ExtractResourcePlugin::<ClearColor>::default())?;

        Ok(())
    }
}
