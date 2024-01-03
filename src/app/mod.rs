use std::cell::RefCell;

use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

use crate::{
    core::{camera::FlyCamera, input::Input, time::Time, transform::Transform, ui::EguiContext},
    ecs::{system::SystemId, Bundle, Entity, Resource, System, World},
    renderer::Renderer,
};

use self::{asset_server::AssetServer, commands::Commands};

pub mod asset_server;
pub mod commands;

pub struct App {
    event_loop: EventLoop<()>,
    input: WinitInputHelper,

    #[allow(dead_code)]
    window: Window,

    pub(crate) world: RefCell<World>,
    pub(crate) asset_server: AssetServer,

    ui: EguiContext,

    fps_frame_count: usize,
    frame_time: std::time::Duration,
    fps_last_update: std::time::Instant,
}

impl App {
    pub fn new(screen_width: usize, screen_height: usize) -> anyhow::Result<Self> {
        let event_loop = EventLoop::new();
        let input = WinitInputHelper::new();
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

        let asset_server = AssetServer::new(&world)?;

        Ok(Self {
            event_loop,
            input,
            window,
            fps_frame_count: 0,
            fps_last_update: std::time::Instant::now(),
            frame_time: std::time::Duration::from_secs(0),
            world: RefCell::new(world),
            asset_server,
            ui,
        })
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) -> anyhow::Result<()> {
        self.world.borrow_mut().insert_resource(resource)
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> anyhow::Result<Entity> {
        bundle.build(&mut self.world.borrow_mut())
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
        let mut world = self.world.borrow_mut();
        let mut commands = Commands::new(&mut world);
        f(&mut commands, &mut self.asset_server)
    }

    pub fn run(mut self) -> anyhow::Result<()> {
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
                world.write_resource::<Time>().unwrap().update();
                self.input.update(&event);
                world.write_resource::<Input>().unwrap().update(&event);
                world.update().unwrap();
            }

            match event {
                winit::event::Event::WindowEvent { event, .. } => {
                    self.ui.handle_input(&event);
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
                    {
                        let world = self.world.borrow();
                        let renderer = world.read_resource::<Renderer>().unwrap();
                        let tick = std::time::Instant::now();
                        let output = renderer.prepare();
                        renderer.render(&world, &output).unwrap();
                        renderer.render_ui(&mut self.ui, &self.window, &output, &world);
                        renderer.present(output);
                        self.frame_time += std::time::Instant::now() - tick;
                    }

                    self.fps_frame_count += 1;
                    if self.fps_last_update.elapsed() > std::time::Duration::from_secs(1) {
                        log::info!("FPS: {}", self.fps_frame_count);
                        log::info!(
                            "Frame time: {}ms",
                            self.frame_time.as_millis() as f32 / self.fps_frame_count as f32
                        );
                        log::info!(
                            "Time spent rendering: {}%",
                            self.frame_time.as_secs_f32() * 100.0
                        );
                        self.fps_frame_count = 0;
                        self.fps_last_update = std::time::Instant::now();
                        self.frame_time = std::time::Duration::from_secs(0);
                    }
                }
                _ => {}
            }
        });
    }
}
