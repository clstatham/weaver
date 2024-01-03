use egui::Context;
use egui_wgpu::renderer::ScreenDescriptor;
use egui_winit::State;
use weaver_proc_macro::Component;
use winit::window::Window;

use super::texture::Texture;

// TODO: Make this whole implemention more ECS-friendly.

//       - Ideally, we'd be able to have the `RunUi` functionality be implementable by any component.
//         This would eliminate the need for the catch-all `UiElement` component.
//         We could then have a `UiSystem` that queries for components that implement `RunUi`.
//         But then, how would we query for components that implement `RunUi`?
//         We'd need to be able to query for components that implement a trait, which is not possible.

//       - We could also just keep track of all components that implement `RunUi` separately in the `World`.
//         But that would be a little hacky.

//       - We could also just have a `run_ui` be a basic functionality of a component, that by default does nothing.
//         But how would we change the `run_ui` function of a component?

pub trait RunUi: Send + Sync + 'static {
    fn run_ui(&mut self, ctx: &Context);
    fn into_element(self) -> UiElement
    where
        Self: Sized,
    {
        UiElement {
            run_ui: Box::new(self),
        }
    }
}

impl<F> RunUi for F
where
    F: FnMut(&Context) + Send + Sync + 'static,
{
    fn run_ui(&mut self, ctx: &Context) {
        self(ctx);
    }
}

#[derive(Component)]
pub struct UiElement {
    pub run_ui: Box<dyn RunUi>,
}

impl RunUi for UiElement {
    fn run_ui(&mut self, ctx: &Context) {
        self.run_ui.run_ui(ctx);
    }
}

pub struct EguiContext {
    pub ctx: Context,
    state: State,
    renderer: egui_wgpu::Renderer,
}

impl EguiContext {
    pub fn new(device: &wgpu::Device, window: &Window, msaa_samples: u32) -> Self {
        let ctx = Context::default();
        let state = State::new(ctx.viewport_id(), window, None, None);
        let renderer = egui_wgpu::Renderer::new(device, Texture::WINDOW_FORMAT, None, msaa_samples);
        Self {
            ctx,
            state,
            renderer,
        }
    }

    pub fn handle_input(&mut self, event: &winit::event::WindowEvent) {
        let _ = self.state.on_window_event(&self.ctx, event);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &Window,
        window_surface_view: &wgpu::TextureView,
        screen_descriptor: &ScreenDescriptor,
        run_ui: impl FnOnce(&Context),
    ) {
        let pixels_per_point = screen_descriptor.pixels_per_point;
        let raw_input = self.state.take_egui_input(window);
        let full_output = self.ctx.run(raw_input, |ui| run_ui(ui));

        self.state
            .handle_platform_output(window, &self.ctx, full_output.platform_output);

        let tris = self.ctx.tessellate(full_output.shapes, pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .update_buffers(device, queue, encoder, &tris, screen_descriptor);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.renderer
            .render(&mut render_pass, &tris, screen_descriptor);
        drop(render_pass);
        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x);
        }
    }
}
