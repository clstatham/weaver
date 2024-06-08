use weaver_app::{plugin::Plugin, prelude::App, Runner};
use weaver_ecs::system::SystemStage;
use winit::{
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

pub mod prelude {
    pub use super::WinitPlugin;
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
        let event_loop = EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_inner_size(LogicalSize::new(self.initial_size.0, self.initial_size.1))
            .build(&event_loop)?;

        app.world().insert_resource(window);
        app.world().insert_resource(event_loop);
        app.set_runner(WinitRunner);
        Ok(())
    }
}

struct WinitRunner;

impl Runner for WinitRunner {
    fn run(&self, app: App) -> anyhow::Result<()> {
        app.run_systems(SystemStage::PreInit)?;
        app.run_systems(SystemStage::Init)?;
        app.run_systems(SystemStage::PostInit)?;

        let event_loop = app.world().remove_resource::<EventLoop<()>>().unwrap();

        event_loop.run(move |event, event_loop_window| match event {
            winit::event::Event::WindowEvent { event, window_id } => {
                if let Some(window) = app.world().get_resource::<Window>() {
                    if window.id() == window_id {
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

                                app.run_systems(SystemStage::Ui).unwrap();

                                app.run_systems(SystemStage::PreRender).unwrap();
                                app.run_systems(SystemStage::Render).unwrap();
                                window.pre_present_notify();
                                app.run_systems(SystemStage::PostRender).unwrap();

                                window.request_redraw();
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => (),
        })?;

        Ok(())
    }
}
