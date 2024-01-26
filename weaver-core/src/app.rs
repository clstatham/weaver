use crate::{
    asset_server::AssetServer, doodads::Doodads, input::Input, scripts::Scripts, time::Time,
    ui::EguiContext,
};

use std::sync::Arc;

use parking_lot::RwLock;
use petgraph::prelude::NodeIndex;
use weaver_ecs::prelude::*;
use winit::{event_loop::EventLoop, window::WindowBuilder};

use crate::renderer::{compute::hdr_loader::HdrLoader, Renderer};

#[derive(Component, Clone)]
pub struct Window {
    window: Arc<winit::window::Window>,
    pub fps_mode: bool,
}

impl Window {
    pub fn set_fps_mode(&mut self, fps_mode: bool) {
        self.fps_mode = fps_mode;
    }
}

pub struct App {
    event_loop: EventLoop<()>,
    pub world: Arc<RwLock<World>>,
}

impl App {
    pub fn new(
        screen_width: usize,
        screen_height: usize,
        #[cfg(feature = "serde")] world_path: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Self> {
        #[cfg(feature = "serde")]
        let world = if let Some(ref world_path) = world_path {
            World::from_json_file(world_path)?
        } else {
            World::new()
        };
        #[cfg(not(feature = "serde"))]
        let world = World::new();
        crate::register_all(world.registry());
        let world = Arc::new(RwLock::new(world));
        let world_sref = world.clone();
        world.write().add_resource(world_sref)?;

        let scripts = Scripts::new(world.clone());
        world.write().add_resource(scripts)?;

        let event_loop = EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_title("Weaver")
            .with_inner_size(winit::dpi::LogicalSize::new(
                screen_width as f64,
                screen_height as f64,
            ))
            .with_resizable(false)
            .build(&event_loop)?;

        let renderer = Renderer::new(&window, world.clone());

        let ui = EguiContext::new(renderer.device(), &window, 1);

        let hdr_loader = HdrLoader::new(renderer.device());

        world.write().add_resource(renderer)?;
        world.write().add_resource(hdr_loader)?;
        world.write().add_resource(Time::new())?;
        world.write().add_resource(Input::default())?;
        world.write().add_resource(ui)?;
        world.write().add_resource(Doodads::default())?;

        let asset_server = AssetServer::new(&world.read())?;

        world.write().add_resource(asset_server)?;

        world.write().add_resource(Window {
            window: Arc::new(window),
            fps_mode: false,
        })?;

        world
            .write()
            .add_system_to_stage(Render, SystemStage::Render);

        Ok(Self { event_loop, world })
    }

    pub fn add_resource<T: Component>(&self, resource: T) -> anyhow::Result<()> {
        self.world.write().add_resource(resource)
    }

    pub fn add_system<T: System + 'static>(&self, system: T) -> NodeIndex {
        self.world.write().add_system(system)
    }

    pub fn add_system_to_stage<T: System + 'static>(
        &self,
        system: T,
        stage: SystemStage,
    ) -> NodeIndex {
        self.world.write().add_system_to_stage(system, stage)
    }

    pub fn add_script(&self, script_path: impl AsRef<std::path::Path>) {
        World::add_script(&self.world, script_path);
    }

    pub fn run(self) -> anyhow::Result<()> {
        World::run_stage(&self.world, SystemStage::Startup)?;

        // ECS update task
        let (killswitch, killswitch_rx) = crossbeam_channel::bounded(1);
        let (window_event_tx, window_event_rx) = crossbeam_channel::unbounded();
        let (device_event_tx, device_event_rx) = crossbeam_channel::unbounded();
        let update_world = self.world.clone();
        std::thread::Builder::new()
            .name("Weaver ECS Update Loop".to_owned())
            .spawn(move || {
                loop {
                    {
                        let world = update_world.read();
                        let mut input = world.write_resource::<Input>().unwrap();
                        input.prepare_for_update();

                        while let Ok(event) = window_event_rx.try_recv() {
                            input.update_window(&event);

                            let window = world.read_resource::<Window>().unwrap();
                            let mut ui = world.write_resource::<EguiContext>().unwrap();
                            ui.handle_input(&window.window, &event);
                        }
                        while let Ok(event) = device_event_rx.try_recv() {
                            input.update_device(&event);
                        }
                    }

                    {
                        let world = update_world.read();
                        let mut time = world.write_resource::<Time>().unwrap();
                        time.update();

                        let window = world.read_resource::<Window>().unwrap();
                        let mut gui = world.write_resource::<EguiContext>().unwrap();
                        gui.begin_frame(&window.window);
                    }

                    World::run_stage(&update_world, SystemStage::PreUpdate).unwrap();

                    World::run_stage(&update_world, SystemStage::Update).unwrap();

                    World::run_stage(&update_world, SystemStage::PostUpdate).unwrap();

                    {
                        let world = update_world.read();
                        let mut gui = world.write_resource::<EguiContext>().unwrap();
                        gui.end_frame();
                    }

                    World::run_stage(&update_world, SystemStage::Render).unwrap();

                    if killswitch_rx.try_recv().is_ok() {
                        break;
                    }

                    {
                        let world = update_world.read();
                        let window = world.read_resource::<Window>().unwrap();
                        window.window.request_redraw();
                    }

                    std::thread::sleep(std::time::Duration::from_millis(1));
                }

                World::run_stage(&update_world, SystemStage::Shutdown).unwrap();
            })?;

        self.event_loop.run(move |event, target| {
            // target.set_control_flow(winit::event_loop::ControlFlow::Poll);

            match event {
                winit::event::Event::LoopExiting => {
                    killswitch.send(()).unwrap();
                }
                winit::event::Event::DeviceEvent { event, .. } => {
                    device_event_tx.send(event.clone()).unwrap();
                }
                winit::event::Event::WindowEvent { event, .. } => {
                    window_event_tx.send(event.clone()).unwrap();
                    match event {
                        winit::event::WindowEvent::CloseRequested => {
                            target.exit();
                        }
                        winit::event::WindowEvent::Resized(_size) => {
                            // todo
                        }
                        winit::event::WindowEvent::CursorMoved { .. } => {
                            // center the cursor
                            let world = self.world.read();
                            let window = world.read_resource::<Window>().unwrap();
                            if window.fps_mode {
                                window
                                    .window
                                    .set_cursor_position(winit::dpi::PhysicalPosition::new(
                                        window.window.inner_size().width / 2,
                                        window.window.inner_size().height / 2,
                                    ))
                                    .unwrap();
                                window
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                                    .unwrap();
                                window.window.set_cursor_visible(false);
                            } else {
                                window
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None)
                                    .unwrap();
                                window.window.set_cursor_visible(true);
                            }
                        }
                        winit::event::WindowEvent::RedrawRequested => {
                            let world = self.world.read();
                            let window = world.read_resource::<Window>().unwrap();
                            let mut renderer = world.write_resource::<Renderer>().unwrap();
                            window.window.pre_present_notify();
                            renderer.present();
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

#[system(Render)]
fn render(renderer: ResMut<Renderer>, window: Res<Window>, ui: ResMut<EguiContext>) {
    if let Some(mut encoder) = renderer.begin_frame() {
        renderer.prepare_components();
        renderer.prepare_passes();
        renderer.render(&mut encoder).unwrap();
        renderer.render_ui(&mut ui, &window.window, &mut encoder);
        renderer.end_frame(encoder);
    }
}
