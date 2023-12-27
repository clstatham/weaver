use weaver_ecs::{Bundle, Entity, Resource, World};
use winit::{event_loop::EventLoop, window::Window};
use winit_input_helper::WinitInputHelper;

use crate::{
    core::{camera::Camera, input::Input, model::Model, time::Time},
    renderer::Renderer,
};

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
        let window = Window::new(&event_loop).unwrap();
        window.set_inner_size(winit::dpi::LogicalSize::new(
            screen_width as f64,
            screen_height as f64,
        ));
        window.set_resizable(false);

        let renderer = pollster::block_on(Renderer::new(&window));

        let mut world = World::new();
        world.insert_resource(Time::new());
        world.insert_resource(Input::new());
        world.insert_resource(Camera::new(
            glam::Vec3::new(5.0, 5.0, 5.0),
            glam::Vec3::new(0.0, 0.0, 0.0),
            glam::Vec3::Y,
            45.0f32.to_radians(),
            screen_width as f32 / screen_height as f32,
            0.1,
            100.0,
        ));

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

    pub fn load_model(&mut self, path: &str) -> anyhow::Result<Entity> {
        let model = Model::load_gltf(path, &self.renderer.device)?;
        Ok(self.spawn(model))
    }

    pub fn run(mut self) {
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;
            self.window.request_redraw();
            self.world.write_resource::<Time>().update();

            self.input.update(&event);
            self.world.write_resource::<Input>().update(&event);

            self.world.update();

            match event {
                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                winit::event::Event::RedrawRequested(_) => {
                    self.renderer.render(&mut self.world).unwrap();

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
