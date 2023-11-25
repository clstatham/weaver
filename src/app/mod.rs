use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use winit::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

use crate::ecs::{
    component::Field,
    system::{Query, ResolvedQuery},
};

/// The main application struct. Contains all the state needed to run the game engine.
pub struct App {
    /// Winit event loop.
    event_loop: EventLoop<()>,
    /// Winit input helper.
    input: WinitInputHelper,
    /// Winit window.
    window: Window,
    /// Pixels framebuffer handle.
    pixels: Pixels,

    /// The renderer.
    pub(crate) renderer: crate::renderer::Renderer,

    /// The time of the last frame.
    last_frame_time: std::time::Instant,

    /// The ECS World.
    pub(crate) world: crate::ecs::world::World,

    /// The GUI context.
    pub(crate) gui: crate::gui::Gui,
}

impl App {
    /// Create a new instance of the game engine, initializing everything needed to run the main loop.
    pub fn new(window_size: (u32, u32), window_title: &str) -> App {
        // Instantiate Winit stuff.
        let event_loop = EventLoop::new();
        let input = WinitInputHelper::new();

        let window = {
            WindowBuilder::new()
                .with_title(window_title)
                .with_inner_size(winit::dpi::LogicalSize::new(window_size.0, window_size.1))
                .with_resizable(false)
                .build(&event_loop)
                .unwrap()
        };

        // Instantiate Pixels framebuffer.
        let window_size = window.inner_size();
        let (pixels, gui) = {
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window);
            let scale_factor = window.scale_factor() as f32;
            let pixels = PixelsBuilder::new(window_size.width, window_size.height, surface_texture)
                .build()
                .unwrap();
            let gui = crate::gui::Gui::new(
                &event_loop,
                window_size.width,
                window_size.height,
                scale_factor,
                &pixels,
            );

            (pixels, gui)
        };

        // Instantiate ECS world.
        let mut world = crate::ecs::world::World::new();

        // Instantiate renderer.
        let renderer = crate::renderer::Renderer::new(window_size.width, window_size.height);

        // Instantiate timer.
        let last_frame_time = std::time::Instant::now();
        // Add timer to the World as an entity/component.
        let timer_entity = world.create_entity();
        let mut timer_component = crate::ecs::component::Component::new("timer".to_string());
        timer_component.add_field("dt", Field::F32(0.0));
        world.add_component(timer_entity, timer_component);

        App {
            event_loop,
            input,
            window,
            pixels,
            world,
            last_frame_time,
            renderer,
            gui,
        }
    }

    /// Runs the main event loop of the game engine.
    #[allow(clippy::needless_return)]
    pub fn run(mut self) -> anyhow::Result<()> {
        self.event_loop.run(move |event, _, control_flow| {
            self.window.request_redraw();

            if self.input.update(&event) {
                // ...
                if self.input.close_requested() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }

            if let Event::WindowEvent { event, .. } = &event {
                self.gui.handle_event(event);
            }

            if let Event::RedrawRequested(_) = event {
                // Update timer.
                let current_time = std::time::Instant::now();
                let dt = current_time
                    .duration_since(self.last_frame_time)
                    .as_secs_f32();
                self.last_frame_time = current_time;
                let timers_query = Query::Mutable("timer".to_string());
                let timers = self.world.query(&timers_query);
                if let ResolvedQuery::Mutable(timer) = timers {
                    for mut timer in timer {
                        if let Some(Field::F32(old_dt)) = timer.fields.get_mut("dt") {
                            *old_dt = dt;
                        } else {
                            log::error!("timer component does not have a f32 dt field");
                        }
                    }
                } else {
                    log::error!("timer component not found");
                }

                self.world.update();

                if let Err(err) = self.renderer.render(self.pixels.frame_mut(), &self.world) {
                    log::error!("renderer.render() failed: {}", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                self.gui.prepare(&self.window);

                if let Err(err) = self.pixels.render_with(|encoder, render_target, context| {
                    context.scaling_renderer.render(encoder, render_target);
                    self.gui.render(encoder, render_target, context);

                    Ok(())
                }) {
                    log::error!("pixels.render() failed: {}", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }
        })
    }
}
