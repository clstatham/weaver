use weaver_ecs::{Bundle, Entity, Resource, System, World};
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

use crate::{
    core::{camera::FlyCamera, input::Input, time::Time, transform::Transform},
    renderer::Renderer,
};

pub mod commands;

pub struct App {
    event_loop: EventLoop<()>,
    input: WinitInputHelper,
    window: Window,

    pub(crate) world: World,

    pub(crate) renderer: Renderer,

    fps_frame_count: usize,
    fps_last_update: std::time::Instant,
}

impl App {
    pub fn new(screen_width: usize, screen_height: usize) -> Self {
        let event_loop = EventLoop::new();
        let input = WinitInputHelper::new();
        let window = WindowBuilder::new()
            .with_title("Weaver")
            .with_inner_size(winit::dpi::LogicalSize::new(
                screen_width as f64,
                screen_height as f64,
            ))
            .with_resizable(false)
            .build(&event_loop)
            .unwrap();

        // let camera = Camera::new(
        //     glam::Vec3::new(5.0, 5.0, 5.0),
        //     glam::Vec3::new(0.0, 0.0, 0.0),
        //     glam::Vec3::Y,
        //     45.0f32.to_radians(),
        //     screen_width as f32 / screen_height as f32,
        //     0.1,
        //     100.0,
        // );

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

        let mut world = World::new();
        world.insert_resource(Time::new());
        world.insert_resource(Input::new());
        world.insert_resource(camera);

        Self {
            event_loop,
            input,
            renderer,
            window,
            fps_frame_count: 0,
            fps_last_update: std::time::Instant::now(),
            world,
        }
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) {
        self.world.insert_resource(resource);
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(&mut self.world)
    }

    pub fn add_system<T: System + 'static>(&mut self, system: T) {
        self.world.add_system(system);
    }

    // todo: this is a temporary workaround until we have proper "setup" systems, and systems can take `Commands` as an argument
    pub fn build<'a, F>(&'a mut self, f: F)
    where
        F: FnOnce(commands::Commands<'a>),
    {
        let commands = commands::Commands::new(&mut self.world, &mut self.renderer);
        f(commands);
    }

    pub fn run(mut self) {
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;
            self.window.request_redraw();
            self.world.write_resource::<Time>().update();
            self.input.update(&event);
            // self.window
            //     .set_cursor_position(PhysicalPosition::new(
            //         self.window.inner_size().width as f64 / 2.0,
            //         self.window.inner_size().height as f64 / 2.0,
            //     ))
            //     .unwrap();

            self.world.write_resource::<Input>().update(&event);

            self.world.update();

            match event {
                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::KeyboardInput { input, .. },
                    ..
                } => {
                    if let Some(key_code) = input.virtual_keycode {
                        if key_code == winit::event::VirtualKeyCode::Escape {
                            *control_flow = winit::event_loop::ControlFlow::Exit;
                        }
                    }
                }
                winit::event::Event::RedrawRequested(_) => {
                    self.renderer.render(&self.world).unwrap();

                    self.fps_frame_count += 1;
                    if self.fps_last_update.elapsed() > std::time::Duration::from_secs(1) {
                        log::info!("FPS: {}", self.fps_frame_count);
                        self.fps_frame_count = 0;
                        self.fps_last_update = std::time::Instant::now();
                    }
                }
                _ => {}
            }
        });
    }
}
