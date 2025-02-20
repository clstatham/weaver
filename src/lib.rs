pub use weaver_app;
pub use weaver_asset;
pub use weaver_core;

pub use weaver_ecs;
pub use weaver_event;
pub use weaver_gizmos;
pub use weaver_pbr;
pub use weaver_renderer;
pub use weaver_util;
pub use weaver_winit;

pub mod prelude {
    pub use super::*;
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
}

pub struct DefaultPlugins;

impl weaver_app::plugin::Plugin for DefaultPlugins {
    fn build(&self, app: &mut weaver_app::App) -> weaver_util::prelude::Result<()> {
        use crate::prelude::*;
        use weaver_core::CoreTypesPlugin;
        use weaver_renderer::clear_color::ClearColorPlugin;

        app.add_plugin(CoreTypesPlugin)?
            .add_plugin(WindowPlugin::default())?
            .add_plugin(WinitPlugin)?
            .add_plugin(TimePlugin)?
            .add_plugin(InputPlugin)?
            .add_plugin(RendererPlugin)?
            .add_plugin(ClearColorPlugin(Color::new(0.1, 0.1, 0.1, 1.0)))?
            .add_plugin(PbrPlugin)?;

        Ok(())
    }
}
