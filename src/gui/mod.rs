use egui::{ClippedPrimitive, Context, TexturesDelta};
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use pixels::PixelsContext;
use winit::{event_loop::EventLoopWindowTarget, window::Window};

pub struct Gui {
    pub ctx: Context,
    pub state: egui_winit::State,
    pub screen_descriptor: ScreenDescriptor,
    pub renderer: Renderer,
    pub paint_jobs: Vec<ClippedPrimitive>,
    pub textures: TexturesDelta,
}

impl Gui {
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        width: u32,
        height: u32,
        scale_factor: f32,
        pixels: &pixels::Pixels,
    ) -> Self {
        let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;

        let ctx = Context::default();
        let mut state = egui_winit::State::new(event_loop);
        state.set_max_texture_side(max_texture_size);
        state.set_pixels_per_point(scale_factor);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let renderer = Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1);
        let textures = TexturesDelta::default();
        Self {
            ctx,
            state,
            screen_descriptor,
            renderer,
            paint_jobs: Vec::new(),
            textures,
        }
    }

    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        let _ = self.state.on_event(&self.ctx, event);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    pub fn scale_factor(&mut self, scale_factor: f32) {
        self.screen_descriptor.pixels_per_point = scale_factor;
    }

    pub fn prepare(&mut self, window: &Window) {
        let raw_input = self.state.take_egui_input(window);
        let output = self.ctx.run(raw_input, |ctx| {
            self.ui(ctx);
        });

        self.textures.append(output.textures_delta);
        self.state
            .handle_platform_output(window, &self.ctx, output.platform_output);
        self.paint_jobs = self.ctx.tessellate(output.shapes);
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &PixelsContext,
    ) {
        for (id, image_delta) in &self.textures.set {
            self.renderer
                .update_texture(&context.device, &context.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &context.device,
            &context.queue,
            encoder,
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // LoadOp::Clear(egui::Rgba::TRANSPARENT),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.renderer
                .render(&mut rpass, &self.paint_jobs, &self.screen_descriptor);
        }

        let textures = std::mem::take(&mut self.textures);
        for id in &textures.free {
            self.renderer.free_texture(id);
        }
    }

    pub fn ui(&self, ctx: &Context) {
        egui::Window::new("Hello world!").show(ctx, |ui| {
            ui.label("Hello world!");
        });
    }
}
