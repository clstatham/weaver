use std::sync::Arc;

use parking_lot::RwLock;
use weaver_proc_macro::system;
use winit::{event_loop::EventLoop, window::WindowBuilder};

use crate::{
    core::{doodads::Doodads, input::Input, time::Time, ui::EguiContext},
    ecs::{
        system::{SystemId, SystemStage},
        Bundle, Entity, Resource, System, World,
    },
    renderer::{compute::hdr_loader::HdrLoader, Renderer},
};

use self::asset_server::AssetServer;

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
        let world = Arc::new(RwLock::new(World::new()));

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

        world.write().insert_resource(renderer)?;
        world.write().insert_resource(hdr_loader)?;
        world.write().insert_resource(Time::new())?;
        world.write().insert_resource(Input::default())?;
        world.write().insert_resource(ui)?;
        world.write().insert_resource(Doodads::default())?;

        let asset_server = AssetServer::new(&world.read())?;

        world.write().insert_resource(asset_server)?;

        world.write().insert_resource(Window {
            window,
            fps_mode: false,
        })?;

        world
            .write()
            .add_system_to_stage(InputReset, SystemStage::PostUpdate);

        world
            .write()
            .add_system_to_stage(Render, SystemStage::PostUpdate);

        Ok(Self { event_loop, world })
    }

    pub fn insert_resource<T: Resource>(&self, resource: T) -> anyhow::Result<()> {
        self.world.write().insert_resource(resource)
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> anyhow::Result<Entity> {
        bundle.build(&self.world.read())
    }

    pub fn add_system<T: System + 'static>(&self, system: T) -> SystemId {
        self.world.write().add_system(system)
    }

    pub fn add_system_to_stage<T: System + 'static>(
        &self,
        system: T,
        stage: SystemStage,
    ) -> SystemId {
        self.world.write().add_system_to_stage(system, stage)
    }

    pub fn run(self) -> anyhow::Result<()> {
        World::run_stage(&self.world, SystemStage::Startup)?;

        // ECS update task
        let (killswitch, killswitch_rx) = crossbeam_channel::bounded(1);
        let update_world = self.world.clone();
        std::thread::spawn(move || {
            loop {
                World::run_stage(&update_world, SystemStage::PreUpdate).unwrap();

                {
                    let world = update_world.read();
                    let mut time = world.write_resource::<Time>().unwrap();
                    time.update();

                    let window = world.read_resource::<Window>().unwrap();
                    let mut gui = world.write_resource::<EguiContext>().unwrap();
                    gui.begin_frame(&window.window);
                }

                World::run_stage(&update_world, SystemStage::Update).unwrap();

                {
                    let world = update_world.read();
                    let mut gui = world.write_resource::<EguiContext>().unwrap();
                    gui.end_frame();
                }

                World::run_stage(&update_world, SystemStage::PostUpdate).unwrap();

                if killswitch_rx.try_recv().is_ok() {
                    break;
                }

                {
                    let world = update_world.read();
                    let window = world.read_resource::<Window>().unwrap();
                    window.window.request_redraw();
                }

                std::thread::yield_now();
            }

            World::run_stage(&update_world, SystemStage::Shutdown).unwrap();
        });

        self.event_loop.run(move |event, target| {
            target.set_control_flow(winit::event_loop::ControlFlow::Poll);

            match event {
                winit::event::Event::LoopExiting => {
                    killswitch.send(()).unwrap();
                }
                winit::event::Event::NewEvents(_cause) => {
                    // let world = self.world.read();
                    // let mut input = world.write_resource::<Input>().unwrap();
                    // input.prepare_for_update();
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

#[system(InputReset)]
fn input_reset(input: ResMut<Input>) {
    input.prepare_for_update();
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
