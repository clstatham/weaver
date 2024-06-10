use std::ops::Deref;

use weaver_app::{plugin::Plugin, prelude::App, Runner};
use weaver_core::input::Input;
use weaver_ecs::{prelude::Component, system::SystemStage, world::World};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    window::WindowBuilder,
};

pub mod prelude {
    pub use super::{WinitEventHooks, WinitPlugin};
    pub use winit;
}

#[derive(Component)]
pub struct Window {
    window: winit::window::Window,
}

impl Deref for Window {
    type Target = winit::window::Window;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

#[derive(Component)]
pub struct EventLoop {
    event_loop: winit::event_loop::EventLoop<()>,
}

#[derive(Default, Component)]
pub struct WinitEventHooks {
    #[allow(clippy::type_complexity)]
    pub on_event: Vec<Box<dyn Fn(&World, &Event<()>)>>,
}

impl WinitEventHooks {
    pub fn push_event_hook<F>(&mut self, f: F)
    where
        F: Fn(&World, &Event<()>) + 'static,
    {
        self.on_event.push(Box::new(f));
    }
}

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

        app.world().insert_resource(Window { window });
        app.world().insert_resource(EventLoop { event_loop });
        app.world().insert_resource(WinitEventHooks::default());
        app.set_runner(WinitRunner);
        Ok(())
    }
}

struct WinitRunner;

impl Runner for WinitRunner {
    fn run(&self, app: &mut App) -> anyhow::Result<()> {
        app.run_systems(SystemStage::PreInit)?;
        app.run_systems(SystemStage::Init)?;
        app.run_systems(SystemStage::PostInit)?;

        let event_loop = app.world().remove_resource::<EventLoop>().unwrap();

        event_loop.event_loop.run(move |event, event_loop_window| {
            event_loop_window.set_control_flow(ControlFlow::Poll);
            if let Some(hooks) = app.world().get_resource::<WinitEventHooks>() {
                for hook in hooks.on_event.iter() {
                    hook(app.world(), &event);
                }
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
                                WindowEvent::CloseRequested => {
                                    app.run_systems(SystemStage::PreShutdown).unwrap();
                                    app.run_systems(SystemStage::Shutdown).unwrap();
                                    app.run_systems(SystemStage::PostShutdown).unwrap();
                                    event_loop_window.exit();
                                }
                                WindowEvent::RedrawRequested => {
                                    app.run_systems(SystemStage::PreUpdate).unwrap();
                                    app.run_systems(SystemStage::Update).unwrap();
                                    app.run_systems(SystemStage::PostUpdate).unwrap();

                                    app.run_systems(SystemStage::PreUi).unwrap();
                                    app.run_systems(SystemStage::Ui).unwrap();
                                    app.run_systems(SystemStage::PostUi).unwrap();

                                    app.run_systems(SystemStage::PreRender).unwrap();
                                    app.run_systems(SystemStage::Render).unwrap();
                                    // window.pre_present_notify();
                                    app.run_systems(SystemStage::PostRender).unwrap();
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
