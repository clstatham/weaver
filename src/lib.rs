//! Weaver, a small Bevy-inspired game engine written for learning.
//!
//! This crate is a facade: it re-exports the workspace crates and provides [`DefaultPlugins`].
//! The engine is split as follows:
//!
//! - [`weaver_util`]: locks, interning, maps, logging macros and error re-exports.
//! - [`weaver_ecs`]: the entity component system, with typed queries, systems, resources and
//!   change detection.
//! - [`weaver_app`]: [`App`](weaver_app::App), plugins, stages and the runner.
//! - [`weaver_event`]: typed event queues.
//! - [`weaver_asset`]: handles, asset storage and loaders.
//! - [`weaver_core`]: transforms, colors, geometry, meshes, textures, time and input.
//! - [`weaver_winit`]: the window and the winit runner.
//! - [`weaver_renderer`]: the wgpu renderer, which runs in its own world on a render thread.
//! - [`weaver_pbr`] and [`weaver_gizmos`]: PBR lighting and materials, and debug drawing.
//!
//! A program builds an [`App`](weaver_app::App), adds [`DefaultPlugins`] plus its own plugins and
//! systems, and calls `run`:
//!
//! ```no_run
//! use weaver::{DefaultPlugins, prelude::*};
//!
//! fn main() -> Result<()> {
//!     App::new()
//!         .add_plugin(DefaultPlugins::new().with_window_title("My game"))?
//!         .add_system(setup, Init)
//!         .run()
//! }
//!
//! fn setup(commands: Commands) {
//!     commands.spawn((Transform::default(),));
//! }
//! ```
//!
//! The [`prelude`] glob-imports the preludes of every crate above (so ECS, app, asset, core,
//! event, gizmos, pbr, renderer, util and winit items are all in scope), plus
//! [`PathBuf`](std::path::PathBuf). `examples/weaver-demo` is a larger working program.

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

/// Everything most programs need, in one glob import.
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

/// The plugin set for a normal windowed program.
///
/// Adds, in order: core asset types, the window settings, the winit runner, time, input, the
/// renderer, a clear color, and PBR. Configure with the `with_*` builders before adding it.
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
    /// Creates the plugin set with default title ("Weaver"), an 800x600 window and a dark gray
    /// clear color.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the window title.
    pub fn with_window_title(mut self, title: &'static str) -> Self {
        self.window_plugin.window_title = title;
        self
    }

    /// Sets the initial window size in pixels.
    pub fn with_window_size(mut self, width: u32, height: u32) -> Self {
        self.window_plugin.initial_size = (width, height);
        self
    }

    /// Sets the color the framebuffer is cleared to each frame.
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
