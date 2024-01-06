use std::cell::RefCell;

use winit::{event_loop::EventLoop, window::WindowBuilder};

use crate::{
    core::{doodads::Doodads, input::Input, time::Time, ui::EguiContext},
    ecs::{system::SystemId, Bundle, Entity, Resource, System, World},
    renderer::Renderer,
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
    pub(crate) world: RefCell<World>,
}

impl App {
    pub fn new(screen_width: usize, screen_height: usize) -> anyhow::Result<Self> {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("Weaver")
            .with_inner_size(winit::dpi::LogicalSize::new(
                screen_width as f64,
                screen_height as f64,
            ))
            .with_resizable(false)
            .build(&event_loop)?;

        window.set_cursor_grab(winit::window::CursorGrabMode::Confined)?;
        window.set_cursor_visible(false);

        let renderer = pollster::block_on(Renderer::new(&window));

        let ui = EguiContext::new(&renderer.device, &window, 1);

        let mut world = World::new();
        world.insert_resource(renderer)?;
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
            world: RefCell::new(world),
        })
    }

    pub fn insert_resource<T: Resource>(&self, resource: T) -> anyhow::Result<()> {
        self.world.borrow_mut().insert_resource(resource)
    }

    pub fn world(&self) -> std::cell::Ref<'_, World> {
        self.world.borrow()
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> anyhow::Result<Entity> {
        bundle.build(&self.world.borrow_mut())
    }

    pub fn add_system<T: System + 'static>(&mut self, system: T) -> SystemId {
        self.world.borrow_mut().add_system(system)
    }

    pub fn add_system_before<T: System + 'static>(
        &mut self,
        system: T,
        before: SystemId,
    ) -> SystemId {
        self.world.borrow_mut().add_system_before(system, before)
    }

    pub fn add_system_after<T: System + 'static>(
        &mut self,
        system: T,
        after: SystemId,
    ) -> SystemId {
        self.world.borrow_mut().add_system_after(system, after)
    }

    pub fn add_system_dependency(&mut self, dependency: SystemId, dependent: SystemId) {
        self.world
            .borrow_mut()
            .add_system_dependency(dependency, dependent);
    }

    pub fn add_startup_system<T: System + 'static>(&mut self, system: T) -> SystemId {
        self.world.borrow_mut().add_startup_system(system)
    }

    pub fn add_startup_system_before<T: System + 'static>(
        &mut self,
        system: T,
        before: SystemId,
    ) -> SystemId {
        self.world
            .borrow_mut()
            .add_startup_system_before(system, before)
    }

    pub fn add_startup_system_after<T: System + 'static>(
        &mut self,
        system: T,
        after: SystemId,
    ) -> SystemId {
        self.world
            .borrow_mut()
            .add_startup_system_after(system, after)
    }

    pub fn add_startup_system_dependency(&mut self, dependency: SystemId, dependent: SystemId) {
        self.world
            .borrow_mut()
            .add_startup_system_dependency(dependency, dependent);
    }

    pub fn build<F>(&mut self, f: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut Commands, &mut AssetServer) -> anyhow::Result<()>,
    {
        let world = self.world.borrow_mut();
        let mut commands = Commands::new(&world);
        let mut asset_server = world.write_resource::<AssetServer>()?;
        f(&mut commands, &mut asset_server)
    }

    pub fn run(self) -> anyhow::Result<()> {
        {
            let world = self.world.borrow();
            world.startup()?;
            let renderer = world.read_resource::<Renderer>()?;
            renderer.prepare_components(&world);
        }

        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;
            {
                let world = self.world.borrow();
                world
                    .read_resource::<Window>()
                    .unwrap()
                    .window
                    .request_redraw();
            }

            match event {
                winit::event::Event::NewEvents(_) => {
                    let world = self.world.borrow();
                    let mut input = world.write_resource::<Input>().unwrap();
                    input.prepare_for_update();
                }
                winit::event::Event::DeviceEvent { event, .. } => {
                    let world = self.world.borrow();
                    let mut input = world.write_resource::<Input>().unwrap();
                    input.update(&event);
                }
                winit::event::Event::WindowEvent { event, .. } => {
                    {
                        let world = self.world.borrow();
                        let mut ui = world.write_resource::<EguiContext>().unwrap();
                        ui.handle_input(&event);
                    }
                    match event {
                        winit::event::WindowEvent::CloseRequested => {
                            *control_flow = winit::event_loop::ControlFlow::Exit;
                        }
                        winit::event::WindowEvent::Resized(_size) => {
                            // todo
                        }
                        winit::event::WindowEvent::CursorMoved { .. } => {
                            // center the cursor
                            let world = self.world.borrow();
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

                        _ => {}
                    }
                }
                winit::event::Event::RedrawRequested(_) => {
                    let world = self.world.borrow();

                    world.write_resource::<Time>().unwrap().update();
                    world
                        .write_resource::<EguiContext>()
                        .unwrap()
                        .begin_frame(&world.read_resource::<Window>().unwrap().window);
                    world.update().unwrap();
                    world.write_resource::<EguiContext>().unwrap().end_frame();

                    let renderer = world.read_resource::<Renderer>().unwrap();
                    let mut ui = world.write_resource::<EguiContext>().unwrap();
                    let output = renderer.prepare();
                    renderer.render(&world, &output).unwrap();
                    renderer.render_ui(
                        &mut ui,
                        &world.read_resource::<Window>().unwrap().window,
                        &output,
                    );
                    renderer.present(output);
                }
                _ => {}
            }
        });
    }
}
