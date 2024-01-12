use std::sync::Arc;

use parking_lot::RwLock;
use winit::{event::WindowEvent, event_loop::EventLoop, window::WindowBuilder};

use crate::{
    core::{doodads::Doodads, input::Input, time::Time, ui::EguiContext},
    ecs::{system::SystemId, Bundle, Entity, Resource, System, World},
    renderer::{compute::hdr_loader::HdrLoader, Renderer},
};

use self::{asset_server::AssetServer, commands::Commands};

pub mod asset_server;
pub mod commands;

#[derive(Resource)]
pub struct Window {
    pub window: winit::window::Window,
    pub fps_mode: bool,
}

impl Window {
    pub fn set_fps_mode(&mut self, fps_mode: bool) {
        self.fps_mode = fps_mode;
    }
}

pub struct App {
    event_loop: EventLoop<()>,
    pub(crate) world: Arc<RwLock<World>>,
}

impl App {
    pub fn new(screen_width: usize, screen_height: usize) -> anyhow::Result<Self> {
        let event_loop = EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_title("Weaver")
            .with_inner_size(winit::dpi::LogicalSize::new(
                screen_width as f64,
                screen_height as f64,
            ))
            .with_resizable(false)
            .build(&event_loop)?;

        let renderer = Renderer::new(&window);

        let ui = EguiContext::new(renderer.device(), &window, 1);

        let hdr_loader = HdrLoader::new(renderer.device());

        let mut world = World::new();
        world.insert_resource(renderer)?;
        world.insert_resource(hdr_loader)?;
        world.insert_resource(Time::new())?;
        world.insert_resource(Input::default())?;
        world.insert_resource(ui)?;
        world.insert_resource(Doodads::default())?;

        let asset_server = AssetServer::new(&world)?;

        world.insert_resource(asset_server)?;

        world.insert_resource(Window {
            window,
            fps_mode: false,
        })?;

        Ok(Self {
            event_loop,
            world: Arc::new(RwLock::new(world)),
        })
    }

    pub fn insert_resource<T: Resource>(&self, resource: T) -> anyhow::Result<()> {
        self.world.write().insert_resource(resource)
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> anyhow::Result<Entity> {
        bundle.build(&self.world.read())
    }

    pub fn add_system<T: System + 'static>(&self, system: T) -> SystemId {
        self.world.read().add_system(system)
    }

    pub fn add_system_before<T: System + 'static>(
        &mut self,
        system: T,
        before: SystemId,
    ) -> SystemId {
        self.world.read().add_system_before(system, before)
    }

    pub fn add_system_after<T: System + 'static>(
        &mut self,
        system: T,
        after: SystemId,
    ) -> SystemId {
        self.world.read().add_system_after(system, after)
    }

    pub fn add_system_dependency(&mut self, dependency: SystemId, dependent: SystemId) {
        self.world
            .read()
            .add_system_dependency(dependency, dependent);
    }

    pub fn add_startup_system<T: System + 'static>(&mut self, system: T) -> SystemId {
        self.world.read().add_startup_system(system)
    }

    pub fn add_startup_system_before<T: System + 'static>(
        &mut self,
        system: T,
        before: SystemId,
    ) -> SystemId {
        self.world.read().add_startup_system_before(system, before)
    }

    pub fn add_startup_system_after<T: System + 'static>(
        &mut self,
        system: T,
        after: SystemId,
    ) -> SystemId {
        self.world.read().add_startup_system_after(system, after)
    }

    pub fn add_startup_system_dependency(&mut self, dependency: SystemId, dependent: SystemId) {
        self.world
            .read()
            .add_startup_system_dependency(dependency, dependent);
    }

    pub fn build<F>(&mut self, f: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut Commands, &mut AssetServer) -> anyhow::Result<()>,
    {
        let world = self.world.read();
        let mut commands = Commands::new(&world);
        let mut asset_server = world.write_resource::<AssetServer>()?;
        f(&mut commands, &mut asset_server)
    }

    pub fn run(self) -> anyhow::Result<()> {
        {
            World::startup(&self.world)?;
        }

        self.event_loop.run(move |event, target| {
            target.set_control_flow(winit::event_loop::ControlFlow::Poll);
            {
                let world = self.world.read();
                world
                    .read_resource::<Window>()
                    .unwrap()
                    .window
                    .request_redraw();
            }

            match event {
                winit::event::Event::NewEvents(_) => {
                    let world = self.world.read();
                    let mut input = world.write_resource::<Input>().unwrap();
                    input.prepare_for_update();
                }
                winit::event::Event::DeviceEvent { event, .. } => {
                    let world = self.world.read();
                    let mut input = world.write_resource::<Input>().unwrap();
                    input.update(&event);
                }
                winit::event::Event::WindowEvent { event, .. } => {
                    {
                        let world = self.world.read();
                        let window = world.read_resource::<Window>().unwrap();
                        let mut ui = world.write_resource::<EguiContext>().unwrap();
                        ui.handle_input(&window.window, &event);
                    }
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

                        WindowEvent::RedrawRequested => {
                            let world = self.world.read();
                            world.write_resource::<Time>().unwrap().update();
                            world
                                .write_resource::<EguiContext>()
                                .unwrap()
                                .begin_frame(&world.read_resource::<Window>().unwrap().window);
                            drop(world);
                            World::update(&self.world).unwrap();
                            let world = self.world.read();
                            world.write_resource::<EguiContext>().unwrap().end_frame();

                            let mut renderer = world.write_resource::<Renderer>().unwrap();
                            let mut ui = world.write_resource::<EguiContext>().unwrap();

                            let (output, mut encoder) = renderer.begin_frame();
                            renderer.prepare_components(&world);
                            renderer.prepare_passes(&world);
                            renderer.render(&world, &output, &mut encoder).unwrap();
                            renderer.render_ui(
                                &mut ui,
                                &world.read_resource::<Window>().unwrap().window,
                                &output,
                                &mut encoder,
                            );
                            renderer.flush_and_present(output, encoder);
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
