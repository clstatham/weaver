use std::cell::RefCell;

use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use crate::{
    core::{
        camera::FlyCamera, doodads::Doodads, input::Input, time::Time, transform::Transform,
        ui::EguiContext,
    },
    ecs::{system::SystemId, Bundle, Entity, Resource, System, World},
    renderer::Renderer,
};

use self::{asset_server::AssetServer, commands::Commands};

pub mod asset_server;
pub mod commands;

pub struct App {
    event_loop: EventLoop<()>,

    #[allow(dead_code)]
    window: Window,

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

        let initial_camera_transform = Transform::default()
            .translate(5.0, 5.0, 5.0)
            .looking_at(glam::Vec3::ZERO, glam::Vec3::Y);
        let camera = FlyCamera {
            speed: 5.0,
            sensitivity: 0.1,
            translation: initial_camera_transform.get_translation(),
            rotation: initial_camera_transform.get_rotation(),
            fov: 90.0f32.to_radians(),
            aspect: screen_width as f32 / screen_height as f32,
            near: 0.1,
            far: 100.0,
        };

        let renderer = pollster::block_on(Renderer::new(&window));

        let ui = EguiContext::new(&renderer.device, &window, 1);

        let mut world = World::new();
        world.insert_resource(renderer)?;
        world.insert_resource(Time::new())?;
        world.insert_resource(Input::new())?;
        world.insert_resource(camera)?;
        world.insert_resource(ui)?;
        world.insert_resource(Doodads::default())?;

        let asset_server = AssetServer::new(&world)?;

        world.insert_resource(asset_server)?;

        Ok(Self {
            event_loop,
            window,
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
            self.window.request_redraw();
            {
                let world = self.world.borrow();

                world
                    .write_resource::<Input>()
                    .unwrap()
                    .input
                    .update(&event);
            }
            match event {
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
                        _ => {}
                    }
                }
                winit::event::Event::RedrawRequested(_) => {
                    let world = self.world.borrow();

                    world.write_resource::<Time>().unwrap().update();
                    world
                        .write_resource::<EguiContext>()
                        .unwrap()
                        .begin_frame(&self.window);
                    world.update().unwrap();
                    world.write_resource::<EguiContext>().unwrap().end_frame();

                    let renderer = world.read_resource::<Renderer>().unwrap();
                    let mut ui = world.write_resource::<EguiContext>().unwrap();
                    let output = renderer.prepare();
                    renderer.render(&world, &output).unwrap();
                    renderer.render_ui(&mut ui, &self.window, &output);
                    renderer.present(output);
                }
                _ => {}
            }
        });
    }
}
