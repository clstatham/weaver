use std::ops::Deref;

use weaver_app::{plugin::Plugin, prelude::App, system::SystemStage, Runner};
use weaver_core::input::Input;
use weaver_ecs::prelude::Resource;
use weaver_util::lock::Lock;
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

#[derive(Resource)]
pub struct Window {
    window: winit::window::Window,
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
    pub initial_size: (u32, u32),
}

impl Default for WinitPlugin {
    fn default() -> Self {
        Self {
            initial_size: (800, 600),
        }
    }
}

impl Plugin for WinitPlugin {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_inner_size(LogicalSize::new(self.initial_size.0, self.initial_size.1))
            .build(&event_loop)?;

        app.insert_resource(Window { window });
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
    fn run(&self, app: &mut App) -> anyhow::Result<()> {
        app.run_systems(SystemStage::PreInit)?;
        app.run_systems(SystemStage::Init)?;
        app.run_systems(SystemStage::PostInit)?;

        let event_loop = self.event_loop.write().take().unwrap();

        event_loop.run(move |event, event_loop_window| {
            event_loop_window.set_control_flow(ControlFlow::Poll);
            if let Some(mut tx) = app
                .world()
                .get_resource_mut::<weaver_event::Events<WinitEvent>>()
            {
                tx.send(WinitEvent {
                    event: event.clone(),
                });
            }
            match event {
                Event::DeviceEvent { event, .. } => {
                    if let Some(mut input) = app.world().get_resource_mut::<Input>() {
                        input.update_device(&event);
                    }
                }
                Event::WindowEvent { event, window_id } => {
                    if let Some(window) = app.world().get_resource::<Window>() {
                        if window.id() == window_id {
                            window.request_redraw();
                            drop(window);

                            if let Some(mut input) = app.world().get_resource_mut::<Input>() {
                                input.update_window(&event);
                            }

                            match event {
                                WindowEvent::Resized(size) => {
                                    let mut tx = app
                                        .world()
                                        .get_resource_mut::<weaver_event::Events<WindowResized>>()
                                        .unwrap();
                                    tx.send(WindowResized {
                                        width: size.width,
                                        height: size.height,
                                    });
                                }
                                WindowEvent::CloseRequested => {
                                    app.run_systems(SystemStage::PreShutdown).unwrap();
                                    app.run_systems(SystemStage::Shutdown).unwrap();
                                    app.run_systems(SystemStage::PostShutdown).unwrap();
                                    event_loop_window.exit();
                                }
                                WindowEvent::RedrawRequested => {
                                    app.world().update();

                                    app.run_systems(SystemStage::PreUpdate).unwrap();
                                    app.run_systems(SystemStage::Update).unwrap();
                                    app.run_systems(SystemStage::PostUpdate).unwrap();

                                    app.run_systems(SystemStage::PreUi).unwrap();
                                    app.run_systems(SystemStage::Ui).unwrap();
                                    app.run_systems(SystemStage::PostUi).unwrap();

                                    app.run_systems(SystemStage::Extract).unwrap();
                                    app.run_systems(SystemStage::PreRender).unwrap();
                                    app.run_systems(SystemStage::Render).unwrap();
                                    app.run_systems(SystemStage::RenderUi).unwrap();
                                    // window.pre_present_notify();
                                    app.run_systems(SystemStage::PostRender).unwrap();

                                    app.run_systems(SystemStage::EventPump).unwrap();
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => (),
            }
        })?;

        Ok(())
    }
}
