use legion::{storage::IntoComponentSource, systems::ParallelRunnable, *};
use winit::{event_loop::EventLoop, window::Window};
use winit_input_helper::WinitInputHelper;

use crate::{
    core::{camera::Camera, input::Input, model::Model, time::Time},
    renderer::Renderer,
};

pub struct AppBuilder {
    width: usize,
    height: usize,
    pub(crate) world: World,
    pub(crate) schedule: legion::systems::Builder,
    pub(crate) resources: Resources,
}

impl AppBuilder {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            world: World::default(),
            schedule: Schedule::builder(),
            resources: Resources::default(),
        }
    }

    pub fn add_system<T: ParallelRunnable + 'static>(&mut self, system: T) -> &mut Self {
        self.schedule.add_system(system);
        self
    }

    pub fn build(mut self) -> App {
        // add default resources
        self.resources.insert(Time::new());
        self.resources.insert(Input::default());
        self.resources.insert(Camera::new(
            glam::Vec3::new(1.0, 1.0, 1.0),
            glam::Vec3::ZERO,
            glam::Vec3::Y,
            90.0f32.to_radians(),
            self.width as f32 / self.height as f32,
            0.001,
            10000.0,
        ));

        // add default systems
        self.schedule
            .add_system(crate::core::time::update_time_system());

        App::new_with_ecs(
            self.width,
            self.height,
            self.world,
            self.schedule.build(),
            self.resources,
        )
    }
}

pub struct App {
    event_loop: EventLoop<()>,
    input: WinitInputHelper,
    window: Window,

    pub(crate) world: World,
    pub(crate) schedule: Schedule,
    pub(crate) resources: Resources,

    pub(crate) renderer: Renderer,

    fps_frame_count: usize,
    fps_last_update: std::time::Instant,
}

impl App {
    pub fn builder(width: usize, height: usize) -> AppBuilder {
        AppBuilder::new(width, height)
    }

    pub fn new_with_ecs(
        screen_width: usize,
        screen_height: usize,
        world: World,
        schedule: Schedule,
        resources: Resources,
    ) -> Self {
        let event_loop = EventLoop::new();
        let input = WinitInputHelper::new();
        let window = Window::new(&event_loop).unwrap();
        window.set_inner_size(winit::dpi::LogicalSize::new(
            screen_width as f64,
            screen_height as f64,
        ));
        window.set_resizable(false);

        let renderer = pollster::block_on(Renderer::new(&window));

        Self {
            event_loop,
            input,
            renderer,
            window,
            fps_frame_count: 0,
            fps_last_update: std::time::Instant::now(),
            schedule,
            resources,
            world,
        }
    }

    pub fn insert_resource<T: Send + Sync + 'static>(&mut self, resource: T) {
        self.resources.insert(resource);
    }

    pub fn spawn<T>(&mut self, components: T) -> Entity
    where
        Option<T>: IntoComponentSource,
    {
        self.world.push(components)
    }

    pub fn load_model(&mut self, path: &str) -> anyhow::Result<Entity> {
        let model = Model::load_gltf(path, &self.renderer.device)?;
        Ok(self.spawn((model,)))
    }

    pub fn run(mut self) {
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;
            self.window.request_redraw();

            self.input.update(&event);
            self.resources.get_mut::<Input>().unwrap().update(&event);

            self.schedule.execute(&mut self.world, &mut self.resources);

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
