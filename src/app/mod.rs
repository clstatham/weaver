use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use raqote::{DrawTarget, SolidSource};
use winit::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

pub struct App {
    event_loop: EventLoop<()>,
    input: WinitInputHelper,
    window: Window,
    pixels: Pixels,
    dt: DrawTarget,
    renderer: crate::renderer::Renderer,
}

impl App {
    pub fn new(window_size: (u32, u32), window_title: &str) -> App {
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

        let window_size = window.inner_size();
        let pixels = {
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window);
            PixelsBuilder::new(window_size.width, window_size.height, surface_texture)
                .texture_format(pixels::wgpu::TextureFormat::Bgra8UnormSrgb) // compat with raqote's DrawTarget
                .build()
                .unwrap()
        };

        let dt = DrawTarget::new(window_size.width as i32, window_size.height as i32);

        let renderer = crate::renderer::Renderer::new();

        App {
            event_loop,
            input,
            window,
            pixels,
            dt,
            renderer,
        }
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        self.event_loop.run(move |event, _, control_flow| {
            if self.input.update(&event) {
                // ...
                if self.input.close_requested() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }

            if let Event::RedrawRequested(_) = event {
                self.dt.clear(SolidSource {
                    r: 50,
                    g: 50,
                    b: 50,
                    a: 255,
                });

                if let Err(err) = self.renderer.render(&mut self.dt) {
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
