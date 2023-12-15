use pixels::Pixels;
use winit::{event_loop::EventLoop, window::Window};
use winit_input_helper::WinitInputHelper;

use crate::ecs::{component::Component, entity::Entity, system::System, world::World};

pub struct App {
    event_loop: EventLoop<()>,
    input: WinitInputHelper,
    window: Window,
    pixels: Pixels,

    pub(crate) world: World,

    last_frame: std::time::Instant,
}

impl App {
    pub fn new() -> Self {
        let event_loop = EventLoop::new();
        let input = WinitInputHelper::new();
        let window = Window::new(&event_loop).unwrap();
        let pixels = {
            let size = window.inner_size();
            let surface_texture = pixels::SurfaceTexture::new(size.width, size.height, &window);
            Pixels::new(800, 600, surface_texture).unwrap()
        };

        let world = World::new();

        Self {
            event_loop,
            input,
            window,
            pixels,
            world,
            last_frame: std::time::Instant::now(),
        }
    }

    pub fn spawn<T: Component>(&mut self, component: T) -> Entity {
        self.world.spawn(component)
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        self.world.add_component(entity, component)
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        self.world.remove_component::<T>(entity)
    }

    pub fn register_system<T: System>(&mut self, system: T) {
        self.world.register_system(system)
    }

    pub fn run(mut self) {
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;
            self.input.update(&event);
            self.window.request_redraw();
            match event {
                winit::event::Event::WindowEvent { event, .. } => match event {
                    winit::event::WindowEvent::CloseRequested => {
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                        return;
                    }
                    _ => {}
                },
                winit::event::Event::RedrawRequested(_) => {
                    let now = std::time::Instant::now();
                    let delta = now - self.last_frame;
                    self.last_frame = now;
                    self.world.update(delta);

                    self.pixels.render().unwrap();
                }
                _ => {}
            }
        });
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
