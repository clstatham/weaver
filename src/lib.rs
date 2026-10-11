pub use weaver_app;
pub use weaver_asset;
pub use weaver_core;
use weaver_core::color::Color;
pub use weaver_ecs;
pub use weaver_event;
pub use weaver_gizmos;
pub use weaver_pbr;
pub use weaver_renderer;
use weaver_renderer::clear_color::ClearColorPlugin;
pub use weaver_util;
pub use weaver_winit;
use weaver_winit::WindowPlugin;

pub mod prelude {
    pub use weaver_app::prelude::*;
    pub use weaver_asset::prelude::*;
    pub use weaver_core::prelude::*;
    pub use weaver_ecs::prelude::*;
    pub use weaver_event::prelude::*;
    pub use weaver_gizmos::prelude::*;
    pub use weaver_pbr::prelude::*;
    pub use weaver_renderer::prelude::*;
    pub use weaver_util::prelude::*;
    pub use weaver_winit::prelude::*;

    pub use std::path::PathBuf;
}

pub struct DefaultPlugins {
    window_plugin: WindowPlugin,
    clear_color_plugin: ClearColorPlugin,
}

impl Default for DefaultPlugins {
    fn default() -> Self {
        Self {
            window_plugin: WindowPlugin::default(),
            clear_color_plugin: ClearColorPlugin(Color::new(0.1, 0.1, 0.1, 1.0)),
        }
    }
}

impl DefaultPlugins {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_window_title(mut self, title: &'static str) -> Self {
        self.window_plugin.window_title = title;
        self
    }

    pub fn with_window_size(mut self, width: u32, height: u32) -> Self {
        self.window_plugin.initial_size = (width, height);
        self
    }

    pub fn with_clear_color(mut self, color: Color) -> Self {
        self.clear_color_plugin = ClearColorPlugin(color);
        self
    }
}

impl weaver_app::plugin::Plugin for DefaultPlugins {
    fn build(&self, app: &mut weaver_app::App) -> weaver_util::prelude::Result<()> {
        use crate::prelude::*;

        app.add_plugin(CoreTypesPlugin)?
            .add_plugin(self.window_plugin.clone())?
            .add_plugin(WinitPlugin)?
            .add_plugin(TimePlugin)?
            .add_plugin(InputPlugin)?
            .add_plugin(RendererPlugin)?
            .add_plugin(self.clear_color_plugin.clone())?
            .add_plugin(PbrPlugin)?;

        Ok(())
    }
}
