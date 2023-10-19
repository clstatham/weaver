use pixels::{wgpu::TextureFormat, PixelsBuilder, SurfaceTexture};
use raqote::{DrawTarget, SolidSource};
use winit::{
    dpi::LogicalSize,
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

pub mod app;
pub mod renderer;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window = {
        WindowBuilder::new()
            .with_title("Weaver")
            .with_inner_size(LogicalSize::new(800, 600))
            .with_resizable(false)
            .build(&event_loop)?
    };
    let window_size = window.inner_size();
    let mut pixels = {
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        PixelsBuilder::new(window_size.width, window_size.height, surface_texture)
            .texture_format(TextureFormat::Bgra8UnormSrgb) // compat with raqote's DrawTarget
            .build()?
    };

    let mut dt = DrawTarget::new(window_size.width as i32, window_size.height as i32);

    let renderer = renderer::Renderer::new();

    event_loop.run(move |event, _, control_flow| {
        if input.update(&event) {
            // ...
            if input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        if let Event::RedrawRequested(_) = event {
            dt.clear(SolidSource {
                r: 50,
                g: 50,
                b: 50,
                a: 255,
            });

            if let Err(err) = renderer.render(&mut dt) {
                log::error!("renderer.render() failed: {}", err);
                *control_flow = ControlFlow::Exit;
                return;
            }

            pixels.frame_mut().copy_from_slice(dt.get_data_u8());
            if let Err(err) = pixels.render() {
                log::error!("pixels.render() failed: {}", err);
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        window.request_redraw();
    });
}
