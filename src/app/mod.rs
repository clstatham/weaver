use pixels::Pixels;
use winit::{event_loop::EventLoop, window::Window};
use winit_input_helper::WinitInputHelper;

use crate::{
    core::input::Input,
    ecs::{
        bundle::Bundle, component::Component, entity::Entity, system::ExclusiveSystem, world::World,
    },
    renderer::Renderer,
};

pub struct App {
    event_loop: EventLoop<()>,
    input: WinitInputHelper,
    window: Window,
    pixels: Pixels,

    pub(crate) world: World,

    pub(crate) renderer: Renderer,

    last_frame: std::time::Instant,

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
        let mut pixels = {
            let size = window.inner_size();
            let surface_texture = pixels::SurfaceTexture::new(size.width, size.height, &window);
            Pixels::new(screen_width as u32, screen_height as u32, surface_texture).unwrap()
        };
        pixels.set_present_mode(wgpu::PresentMode::Immediate);
        pixels.enable_vsync(false);

        let mut world = World::new();
        world.insert_resource(Input::default());

        Self {
            event_loop,
            input,
            window,
            pixels,
            world,
            last_frame: std::time::Instant::now(),
            renderer: Renderer::new(screen_width, screen_height),
            fps_frame_count: 0,
            fps_last_update: std::time::Instant::now(),
        }
    }

    pub fn spawn<T: Component>(&mut self, component: T) -> Entity {
        self.world.spawn(component)
    }

    pub fn build<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(&mut self.world)
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        self.world.add_component(entity, component)
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        self.world.remove_component::<T>(entity)
    }

    pub fn register_system<T: ExclusiveSystem>(&mut self, system: T) {
        self.world.register_system(system)
    }

    pub fn run(mut self) {
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;
            self.window.request_redraw();

            self.input.update(&event);

            self.world.write_resource::<Input>().unwrap().update(&event);

            let now = std::time::Instant::now();
            let delta = now - self.last_frame;
            self.last_frame = now;

            self.world.update(delta);

            match event {
                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                winit::event::Event::RedrawRequested(_) => {
                    self.renderer.render(&mut self.world);

                    let frame = self.pixels.frame_mut();

                    for (i, color) in self.renderer.color_buffer().iter().enumerate() {
                        let (r, g, b) = color.rgb_int();
                        frame[i * 4] = r;
                        frame[i * 4 + 1] = g;
                        frame[i * 4 + 2] = b;
                        frame[i * 4 + 3] = 255;
                    }

                    self.pixels.render().unwrap();

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
