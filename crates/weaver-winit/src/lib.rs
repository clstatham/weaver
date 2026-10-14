//! The winit-based window and runner.
//!
//! [`WindowPlugin`] stores the requested title and size as [`WindowSettings`]. [`WinitPlugin`]
//! installs a [`Runner`] that takes over the main thread:
//!
//! - it calls `App::init`, then starts the winit event loop;
//! - when the loop is resumed it creates the window, inserts it as the [`Window`] resource and
//!   calls `App::finish_plugins`;
//! - window and device events are forwarded into the app as [`WinitEvent`]s and fed to
//!   [`Input`]; resizes also become [`WindowResized`] events and update [`WindowSettings`];
//! - the loop polls continuously and every event requests a redraw, and each
//!   `RedrawRequested` drives one `App::update`;
//! - `CloseRequested` shuts the app down and exits the loop.

use std::{ops::Deref, sync::Arc};

use weaver_app::{Runner, plugin::Plugin, prelude::App};
use weaver_core::input::Input;
use weaver_util::prelude::*;
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, OwnedDisplayHandle},
    window::WindowAttributes,
};

/// Re-exports the window types and `winit` itself.
pub mod prelude {
    pub use super::{Window, WindowPlugin, WindowResized, WindowSettings, WinitEvent, WinitPlugin};
    pub use winit;
}

/// The main window, stored as a resource once the event loop has been resumed.
///
/// Derefs to [`winit::window::Window`]. Cloning is cheap (the window is behind an `Arc`).
#[derive(Clone)]
pub struct Window {
    window: Arc<winit::window::Window>,
    /// Handle to the display connection, for creating graphics surfaces.
    pub display_handle: OwnedDisplayHandle,
}

/// Title and size for the window, inserted by [`WindowPlugin`].
///
/// The runner reads it when it creates the window and then keeps `width` and `height` in sync
/// with the actual size.
#[derive(Debug, Clone)]
pub struct WindowSettings {
    /// Window title.
    pub title: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Deref for Window {
    type Target = winit::window::Window;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

/// A raw winit window or device event, forwarded into the app's event queue.
#[derive(Debug, Clone)]
pub struct WinitEvent {
    /// The event as winit delivered it.
    pub event: Event<()>,
}

/// Sent when the window changes size.
#[derive(Debug, Clone)]
pub struct WindowResized {
    /// New width in pixels.
    pub width: u32,
    /// New height in pixels.
    pub height: u32,
}

/// Plugin that records the window title and initial size and registers [`WindowResized`].
///
/// Doesn't create a window itself; that happens in the runner from [`WinitPlugin`].
#[derive(Clone)]
pub struct WindowPlugin {
    /// Window title.
    pub window_title: &'static str,
    /// Initial inner size as `(width, height)`.
    pub initial_size: (u32, u32),
}

impl Default for WindowPlugin {
    fn default() -> Self {
        Self {
            window_title: "Weaver",
            initial_size: (800, 600),
        }
    }
}

impl WindowPlugin {
    /// Creates the plugin with the given `(width, height)` and title.
    pub fn new(initial_size: (u32, u32), window_title: &'static str) -> Self {
        Self {
            window_title,
            initial_size,
        }
    }
}

impl Plugin for WindowPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.main_app().world().insert_resource(WindowSettings {
            title: self.window_title.to_string(),
            width: self.initial_size.0,
            height: self.initial_size.1,
        });
        app.add_event::<WindowResized>();
        Ok(())
    }

    fn cleanup(&self, app: &mut App) -> Result<()> {
        app.remove_event::<WindowResized>();
        let world = app.main_app().world();
        world.remove_resource::<WindowSettings>();
        Ok(())
    }
}

/// Plugin that replaces the app's runner with a winit event loop and registers [`WinitEvent`].
///
/// Creating the event loop can fail (no display, for instance), in which case `build` returns the
/// error. Needs [`WindowPlugin`] to have run first, since the runner reads [`WindowSettings`].
pub struct WinitPlugin;

impl Plugin for WinitPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        app.set_runner(WinitRunner {
            event_loop: Lock::new(Some(event_loop)),
        });
        app.add_event::<WinitEvent>();
        Ok(())
    }
}

struct WinitRunner {
    event_loop: Lock<Option<winit::event_loop::EventLoop<()>>>,
}

impl Runner for WinitRunner {
    fn run(&self, app: &mut App) -> Result<()> {
        app.init();

        let event_loop = self.event_loop.write().take().unwrap();

        let window_settings = app
            .main_app()
            .world()
            .get_resource::<WindowSettings>()
            .unwrap();

        let mut winit_app = WinitRunnerApp {
            app,
            window_title: window_settings.title.clone(),
            initial_size: (window_settings.width, window_settings.height),
        };

        event_loop.run_app(&mut winit_app)?;

        Ok(())
    }
}

struct WinitRunnerApp<'app> {
    app: &'app mut App,
    window_title: String,
    initial_size: (u32, u32),
}

impl winit::application::ApplicationHandler for WinitRunnerApp<'_> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title(&self.window_title)
                    .with_inner_size(LogicalSize::new(self.initial_size.0, self.initial_size.1)),
            )
            .unwrap();
        let display_handle = event_loop.owned_display_handle();
        let window = Window {
            window: Arc::new(window),
            display_handle,
        };
        if self.app.main_app().world().has_resource::<Window>() {
            self.app.main_app().world().remove_resource::<Window>();
        }
        self.app.main_app().world().insert_resource(window);

        self.app.finish_plugins();
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        event_loop.set_control_flow(ControlFlow::Poll);

        self.app.send_event(WinitEvent {
            event: Event::DeviceEvent {
                device_id,
                event: event.clone(),
            },
        });

        if let Some(mut input) = self.app.main_app().world().get_resource_mut::<Input>() {
            input.update_device(&event);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        event_loop.set_control_flow(ControlFlow::Poll);

        self.app.send_event(WinitEvent {
            event: Event::WindowEvent {
                window_id,
                event: event.clone(),
            },
        });

        if let Some(window) = self.app.main_app().world().get_resource::<Window>() {
            if window.id() != window_id {
                return;
            }

            window.request_redraw();
        }

        if let Some(mut input) = self.app.main_app().world().get_resource_mut::<Input>() {
            input.update_window(&event);
        }

        match event {
            WindowEvent::Resized(size) => {
                self.app.send_event(WindowResized {
                    width: size.width,
                    height: size.height,
                });

                if let Some(mut window_size) = self
                    .app
                    .main_app()
                    .world()
                    .get_resource_mut::<WindowSettings>()
                {
                    window_size.width = size.width;
                    window_size.height = size.height;
                }
            }
            WindowEvent::CloseRequested => {
                self.app.shutdown();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.app.update();
            }
            _ => {}
        }
    }
}
