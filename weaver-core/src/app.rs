use crate::{
    asset_server::AssetServer,
    doodads::Doodads,
    ecs::{
        component::Component,
        storage::{Mut, Ref},
        world::World,
    },
    input::Input,
    prelude::Scene,
    renderer::render_system,
    system::{System, SystemStage},
    time::Time,
    ui::EguiContext,
    util::lock::SharedLock,
};

use std::{rc::Rc, sync::Arc};

use rustc_hash::FxHashMap;
use winit::{event_loop::EventLoop, window::WindowBuilder};

use crate::renderer::{compute::hdr_loader::HdrLoader, Renderer};

#[derive(Clone)]
pub struct Window {
    pub(crate) window: Arc<winit::window::Window>,
    pub fps_mode: bool,
}

impl Window {
    pub fn set_fps_mode(&mut self, fps_mode: bool) {
        self.fps_mode = fps_mode;
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

pub struct App {
    event_loop: Option<EventLoop<()>>,
    pub world: Rc<World>,
    pub root_scene: Rc<Scene>,
    systems: SharedLock<FxHashMap<SystemStage, Vec<Arc<dyn System>>>>,
}

impl App {
    pub fn new(
        title: impl Into<String>,
        screen_width: usize,
        screen_height: usize,
        vsync: bool,
    ) -> anyhow::Result<Self> {
        let world = Rc::new(World::new());

        let event_loop = EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                screen_width as f64,
                screen_height as f64,
            ))
            .with_resizable(true)
            .build(&event_loop)?;

        let renderer = Renderer::new(vsync, &window, world.clone());

        let ui = EguiContext::new(renderer.device(), &window, 1);

        let hdr_loader = HdrLoader::new(renderer.device());

        world.insert_resource(renderer);
        world.insert_resource(hdr_loader);
        world.insert_resource(Time::new());
        world.insert_resource(Input::default());
        world.insert_resource(ui);
        world.insert_resource(Doodads::default());

        let asset_server = AssetServer::new(&world)?;
        world.insert_resource(asset_server);

        world.insert_resource(Window {
            window: Arc::new(window),
            fps_mode: false,
        });

        let root_scene = Rc::new(Scene::new(world.clone()));

        let this = Self {
            event_loop: Some(event_loop),
            world,
            root_scene,
            systems: SharedLock::new(FxHashMap::default()),
        };

        this.add_system(render_system, SystemStage::Render)?;

        Ok(this)
    }

    pub fn add_resource<T: Component>(&self, resource: T) {
        self.world.insert_resource(resource)
    }

    pub fn get_resource<T: Component>(&self) -> Option<Ref<T>> {
        self.world.get_resource::<T>()
    }

    pub fn get_resource_mut<T: Component>(&self) -> Option<Mut<T>> {
        self.world.get_resource_mut::<T>()
    }

    pub fn world(&self) -> &Rc<World> {
        &self.world
    }

    pub fn root_scene(&self) -> &Rc<Scene> {
        &self.root_scene
    }

    pub fn add_system<T: System>(&self, system: T, stage: SystemStage) -> anyhow::Result<()> {
        let system = Arc::new(system);
        self.systems.write().entry(stage).or_default().push(system);
        Ok(())
    }

    pub fn run_systems(&self, stage: SystemStage) -> anyhow::Result<()> {
        let systems = self.systems.read().get(&stage).cloned();
        if let Some(systems) = systems {
            for system in systems {
                system.run(&self.root_scene)?;
            }
        }
        Ok(())
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = self.event_loop.take().unwrap();

        self.run_systems(SystemStage::PreInit).unwrap();
        self.run_systems(SystemStage::Init).unwrap();
        self.run_systems(SystemStage::PostInit).unwrap();

        let (window_event_tx, window_event_rx) = crossbeam_channel::unbounded();
        let (device_event_tx, device_event_rx) = crossbeam_channel::unbounded();
        event_loop.run(move |event, target| {
            target.set_control_flow(winit::event_loop::ControlFlow::Poll);

            match event {
                winit::event::Event::LoopExiting => {
                    self.run_systems(SystemStage::PreShutdown).unwrap();
                    self.run_systems(SystemStage::Shutdown).unwrap();
                    self.run_systems(SystemStage::PostShutdown).unwrap();
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
                        winit::event::WindowEvent::Resized(size) => {
                            let renderer = self.world.get_resource::<Renderer>().unwrap();
                            renderer.resize_surface(size.width, size.height);
                        }
                        winit::event::WindowEvent::CursorMoved { .. } => {
                            // center the cursor
                            let window = self.world.get_resource::<Window>().unwrap();
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
                            {
                                let mut input = self.world.get_resource_mut::<Input>().unwrap();
                                input.prepare_for_update();

                                while let Ok(event) = window_event_rx.try_recv() {
                                    input.update_window(&event);

                                    let window = self.world.get_resource::<Window>().unwrap();
                                    let ui = self.world.get_resource::<EguiContext>().unwrap();
                                    ui.handle_input(&window.window, &event);
                                }
                                while let Ok(event) = device_event_rx.try_recv() {
                                    input.update_device(&event);
                                }
                            }

                            {
                                let mut time = self.world.get_resource_mut::<Time>().unwrap();
                                time.update();
                            }

                            self.run_systems(SystemStage::PreUpdate).unwrap();
                            self.run_systems(SystemStage::Update).unwrap();
                            self.run_systems(SystemStage::PostUpdate).unwrap();

                            {
                                let window = self.world.get_resource::<Window>().unwrap();
                                let gui = self.world.get_resource::<EguiContext>().unwrap();
                                gui.begin_frame(&window.window);
                            }

                            self.run_systems(SystemStage::Ui).unwrap();

                            {
                                let gui = self.world.get_resource::<EguiContext>().unwrap();
                                gui.end_frame();
                            }

                            self.run_systems(SystemStage::PreRender).unwrap();
                            self.run_systems(SystemStage::Render).unwrap();
                            self.run_systems(SystemStage::PostRender).unwrap();

                            {
                                let window = self.world.get_resource::<Window>();
                                let renderer = self.world.get_resource::<Renderer>();
                                if let (Some(window), Some(renderer)) = (window, renderer) {
                                    window.window.pre_present_notify();
                                    renderer.present();
                                    window.request_redraw();
                                };
                            }
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
