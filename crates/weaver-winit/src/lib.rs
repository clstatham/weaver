use std::{ops::Deref, sync::Arc};

use weaver_app::{plugin::Plugin, prelude::App, Runner};
use weaver_core::input::Input;
use weaver_util::prelude::*;
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    window::WindowAttributes,
};

pub mod prelude {
    pub use super::{Window, WindowPlugin, WindowResized, WindowSettings, WinitEvent, WinitPlugin};
    pub use winit;
}

#[derive(Clone)]
pub struct Window {
    window: Arc<winit::window::Window>,
}

#[derive(Debug, Clone)]
pub struct WindowSettings {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Deref for Window {
    type Target = winit::window::Window;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

#[derive(Debug)]
pub struct WinitEvent {
    pub event: Event<()>,
}
impl weaver_event::Event for WinitEvent {}

#[derive(Debug)]
pub struct WindowResized {
    pub width: u32,
    pub height: u32,
}
impl weaver_event::Event for WindowResized {}

pub struct WindowPlugin {
    pub window_title: &'static str,
    pub initial_size: (u32, u32),
}

pub struct WinitPlugin;

impl Default for WindowPlugin {
    fn default() -> Self {
        Self {
            window_title: "Weaver",
            initial_size: (800, 600),
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
        app.main_app().world().remove_resource::<WindowSettings>();
        Ok(())
    }
}

impl Plugin for WinitPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        app.set_runner(WinitRunner {
            event_loop: Lock::new(Some(event_loop)),
        });
        app.add_event::<WinitEvent>();
        app.add_event::<WindowResized>();
        Ok(())
    }
}

struct WinitRunner {
    event_loop: Lock<Option<winit::event_loop::EventLoop<()>>>,
}

impl Runner for WinitRunner {
    fn run(&self, app: &mut App) -> Result<()> {
        pollster::block_on(app.init());

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
        let window = Window {
            window: Arc::new(window),
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
                pollster::block_on(self.app.shutdown());
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                pollster::block_on(self.app.update());
            }
            _ => {}
        }
    }
}
