use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use raqote::{DrawTarget, SolidSource};
use winit::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

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
    /// Raqote draw target.
    dt: DrawTarget,
    /// The camera.
    pub(crate) camera: crate::renderer::camera::PerspectiveCamera,

    /// The ECS World.
    pub(crate) world: crate::ecs::world::World,
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
        let pixels = {
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window);
            PixelsBuilder::new(window_size.width, window_size.height, surface_texture)
                .texture_format(pixels::wgpu::TextureFormat::Bgra8UnormSrgb) // compat with raqote's DrawTarget
                .build()
                .unwrap()
        };

        // Instantiate Raqote draw target and Weaver renderer.
        let dt = DrawTarget::new(window_size.width as i32, window_size.height as i32);

        // Instantiate ECS world.
        let world = crate::ecs::world::World::new();

        // Instantiate camera.
        let mut camera = crate::renderer::camera::PerspectiveCamera::new();
        camera.aspect = window_size.width as f32 / window_size.height as f32;
        camera.position = glam::Vec3::new(0.0, 0.0, 1.0);

        App {
            event_loop,
            input,
            window,
            pixels,
            dt,
            world,
            camera,
        }
    }

    /// Runs the main event loop of the game engine.
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

            self.world.update();

            if let Event::RedrawRequested(_) = event {
                self.dt.clear(SolidSource {
                    r: 50,
                    g: 50,
                    b: 50,
                    a: 255,
                });

                let screen_size = self.window.inner_size();

                if let Err(err) = crate::renderer::render(
                    &mut self.dt,
                    &self.camera,
                    &self.world,
                    (screen_size.width, screen_size.height),
                ) {
                    log::error!("renderer.render() failed: {}", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                self.pixels
                    .frame_mut()
                    .copy_from_slice(self.dt.get_data_u8());
                if let Err(err) = self.pixels.render() {
                    log::error!("pixels.render() failed: {}", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }
        })
    }
}
