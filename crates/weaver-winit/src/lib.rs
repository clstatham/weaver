use std::{ops::Deref, sync::Arc};

use weaver_app::{plugin::Plugin, prelude::App, Runner};
use weaver_core::input::Input;
use weaver_ecs::prelude::Resource;
use weaver_event::ManuallyUpdatedEvents;
use weaver_util::{lock::Lock, prelude::Result};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    window::WindowBuilder,
};

pub mod prelude {
    pub use super::{WindowResized, WinitEvent, WinitPlugin};
    pub use winit;
}

#[derive(Resource, Clone)]
pub struct Window {
    window: Arc<winit::window::Window>,
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

pub struct WinitPlugin {
    pub window_title: &'static str,
    pub initial_size: (u32, u32),
}

impl Default for WinitPlugin {
    fn default() -> Self {
        Self {
            window_title: "Weaver",
            initial_size: (800, 600),
        }
    }
}

impl Plugin for WinitPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_title(self.window_title)
            .with_resizable(false)
            .with_inner_size(LogicalSize::new(self.initial_size.0, self.initial_size.1))
            .build(&event_loop)?;

        app.insert_resource(Window {
            window: Arc::new(window),
        });
        app.set_runner(WinitRunner {
            event_loop: Lock::new(Some(event_loop)),
        });
        app.add_event::<WinitEvent>();
        app.add_manually_updated_event::<WindowResized>();
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

        event_loop.run(move |event, event_loop_window| {
            event_loop_window.set_control_flow(ControlFlow::Poll);

            if let Some(tx) = app
                .main_app_mut()
                .get_resource_mut::<weaver_event::Events<WinitEvent>>()
            {
                tx.send(WinitEvent {
                    event: event.clone(),
                });
            }

            match &event {
                Event::DeviceEvent { event, .. } => {
                    if let Some(mut input) = app.main_app_mut().get_resource_mut::<Input>() {
                        input.update_device(event);
                    }
                }
                Event::WindowEvent { event, window_id } => {
                    if let Some(window) = app.main_app_mut().get_resource::<Window>() {
                        if window.id() != *window_id {
                            return;
                        }

                        window.request_redraw();
                    }

                    if let Some(mut input) = app.main_app_mut().get_resource_mut::<Input>() {
                        input.update_window(event);
                    }

                    match event {
                        WindowEvent::Resized(size) => {
                            let tx = app
                                .main_app_mut()
                                .get_resource_mut::<ManuallyUpdatedEvents<WindowResized>>()
                                .unwrap();
                            tx.send(WindowResized {
                                width: size.width,
                                height: size.height,
                            });
                        }
                        WindowEvent::CloseRequested => {
                            app.shutdown();
                            event_loop_window.exit();
                        }
                        WindowEvent::RedrawRequested => {
                            app.update();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        })?;

        Ok(())
    }
}
