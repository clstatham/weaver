use crate::{
    asset_server::AssetServer, doodads::Doodads, input::Input, time::Time, ui::EguiContext,
};

use std::sync::Arc;

use fabricate::prelude::*;

use winit::{event_loop::EventLoop, window::WindowBuilder};

use crate::renderer::{compute::hdr_loader::HdrLoader, Renderer};

#[derive(Clone, Component)]
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
    event_loop: EventLoop<()>,
    pub world: LockedWorldHandle,
}

impl App {
    pub fn new(
        title: impl Into<String>,
        screen_width: usize,
        screen_height: usize,
        vsync: bool,
    ) -> anyhow::Result<Self> {
        crate::register_names();

        let world = World::new_handle();

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

        world.add_resource(renderer)?;
        world.add_resource(hdr_loader)?;
        world.add_resource(Time::new())?;
        world.add_resource(Input::default())?;
        world.add_resource(ui)?;
        world.add_resource(Doodads::default())?;

        let asset_server = world.defer(|world, _| AssetServer::new(world))??;
        world.add_resource(asset_server)?;

        world.add_resource(Window {
            window: Arc::new(window),
            fps_mode: false,
        })?;

        Ok(Self {
            event_loop,
            world: world.clone(),
        })
    }

    pub fn add_resource<T: Component>(&self, resource: T) -> anyhow::Result<()> {
        self.world.add_resource(resource)
    }

    pub fn add_system<T: System>(&self, system: T, stage: SystemStage) -> anyhow::Result<()> {
        self.world.add_system(stage, system)?;
        Ok(())
    }

    pub fn add_script(&self, script_path: impl AsRef<std::path::Path>) {
        self.world
            .add_script(Script::load(script_path.as_ref()).unwrap());
    }

    pub fn run(self) -> anyhow::Result<()> {
        self.world.run_systems(SystemStage::Startup).unwrap();

        let (window_event_tx, window_event_rx) = crossbeam_channel::unbounded();
        let (device_event_tx, device_event_rx) = crossbeam_channel::unbounded();
        self.event_loop.run(move |event, target| {
            target.set_control_flow(winit::event_loop::ControlFlow::Poll);

            match event {
                winit::event::Event::LoopExiting => {
                    self.world.run_systems(SystemStage::Shutdown).unwrap();
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
                            self.world
                                .with_resource::<Renderer, _, _>(|r| {
                                    r.resize_surface(size.width, size.height)
                                })
                                .unwrap();
                        }
                        winit::event::WindowEvent::CursorMoved { .. } => {
                            // center the cursor
                            self.world
                                .with_resource::<Window, _, _>(|window| {
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
                                            .set_cursor_grab(
                                                winit::window::CursorGrabMode::Confined,
                                            )
                                            .unwrap();
                                        window.window.set_cursor_visible(false);
                                    } else {
                                        window
                                            .window
                                            .set_cursor_grab(winit::window::CursorGrabMode::None)
                                            .unwrap();
                                        window.window.set_cursor_visible(true);
                                    }
                                })
                                .unwrap();
                        }
                        winit::event::WindowEvent::RedrawRequested => {
                            {
                                self.world
                                    .defer(|world, _| {
                                        let mut input = world.write_resource::<Input>().unwrap();
                                        input.prepare_for_update();

                                        while let Ok(event) = window_event_rx.try_recv() {
                                            input.update_window(&event);

                                            let window = world.read_resource::<Window>().unwrap();
                                            let ui = world.read_resource::<EguiContext>().unwrap();
                                            ui.handle_input(&window.window, &event);
                                        }
                                        while let Ok(event) = device_event_rx.try_recv() {
                                            input.update_device(&event);
                                        }
                                    })
                                    .unwrap();
                            }

                            {
                                self.world
                                    .with_resource_mut::<Time, _, _>(|mut time| time.update())
                                    .unwrap();
                            }

                            self.world.run_systems(SystemStage::PreUpdate).unwrap();

                            self.world.run_systems(SystemStage::Update).unwrap();

                            self.world.run_systems(SystemStage::PostUpdate).unwrap();

                            {
                                self.world
                                    .defer(|world, _| {
                                        let window = world.read_resource::<Window>().unwrap();
                                        let gui = world.read_resource::<EguiContext>().unwrap();
                                        gui.begin_frame(&window.window);
                                    })
                                    .unwrap();
                            }

                            self.world.run_systems(SystemStage::Ui).unwrap();

                            {
                                self.world
                                    .defer(|world, _| {
                                        let gui = world.read_resource::<EguiContext>().unwrap();
                                        gui.end_frame();
                                    })
                                    .unwrap();
                            }

                            self.world.run_systems(SystemStage::PreRender).unwrap();

                            self.world.run_systems(SystemStage::Render).unwrap();

                            self.world.run_systems(SystemStage::PostRender).unwrap();

                            {
                                self.world
                                    .defer(|world, _| {
                                        let window = world.read_resource::<Window>();
                                        let renderer = world.read_resource::<Renderer>();
                                        if let (Some(window), Some(renderer)) = (window, renderer) {
                                            window.window.pre_present_notify();
                                            renderer.present();
                                            window.request_redraw();
                                        };
                                    })
                                    .unwrap();
                            }

                            {
                                self.world.garbage_collect().unwrap();
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
