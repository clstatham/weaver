use std::sync::Arc;

use egui::{Context, FullOutput};
use egui_wgpu::{Renderer, ScreenDescriptor};
use egui_winit::{winit, State};
use weaver_app::{plugin::Plugin, system::SystemStage, App};
use weaver_ecs::{component::Res, prelude::Resource, world::World};
use weaver_event::EventRx;
use weaver_renderer::prelude::wgpu;
use weaver_util::{lock::SharedLock, prelude::Result};
use weaver_winit::{Window, WinitEvent};

pub mod prelude {
    pub use super::{EguiContext, EguiPlugin};
    pub use egui;
}

#[derive(Resource)]
pub struct EguiContext {
    state: SharedLock<State>,
    renderer: SharedLock<Renderer>,
    full_output: SharedLock<Option<FullOutput>>,
}

impl EguiContext {
    pub fn new(device: &wgpu::Device, window: &winit::window::Window, msaa_samples: u32) -> Self {
        let ctx = Context::default();
        let viewport_id = ctx.viewport_id();
        let state = State::new(ctx, viewport_id, window, None, None);
        let renderer = Renderer::new(device, wgpu::TextureFormat::Bgra8Unorm, None, msaa_samples);
        Self {
            state: SharedLock::new(state),
            renderer: SharedLock::new(renderer),
            full_output: SharedLock::new(None),
        }
    }

    pub fn available_rect(&self) -> egui::Rect {
        self.state.read().egui_ctx().available_rect()
    }

    pub fn handle_input(&self, window: &winit::window::Window, event: &winit::event::WindowEvent) {
        let _ = self.state.write().on_window_event(window, event);
    }

    pub fn begin_frame(&self, window: &winit::window::Window) {
        if self.full_output.read().is_none() {
            let raw_input = self.state.write().take_egui_input(window);
            self.state.read().egui_ctx().begin_frame(raw_input);
        }
    }

    pub fn end_frame(&self) {
        if self.full_output.read().is_none() {
            *self.full_output.write() = Some(self.state.read().egui_ctx().end_frame());
        }
    }

    pub fn draw_if_ready<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&egui::Context) -> R,
    {
        if self.full_output.read().is_none() {
            Some(f(self.state.read().egui_ctx()))
        } else {
            None
        }
    }

    pub fn convert_texture(
        &self,
        device: &wgpu::Device,
        texture: &wgpu::TextureView,
    ) -> egui::epaint::TextureId {
        self.renderer
            .write()
            .register_native_texture(device, texture, wgpu::FilterMode::Nearest)
    }

    pub fn update_texture(
        &self,
        device: &wgpu::Device,
        texture: &wgpu::TextureView,
        id: egui::epaint::TextureId,
    ) {
        self.renderer.write().update_egui_texture_from_wgpu_texture(
            device,
            texture,
            wgpu::FilterMode::Nearest,
            id,
        );
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &winit::window::Window,
        window_surface_view: &wgpu::TextureView,
        screen_descriptor: &ScreenDescriptor,
    ) {
        if self.full_output.read().is_none() {
            return;
        }
        let full_output = self.full_output.write().take().unwrap();
        let pixels_per_point = screen_descriptor.pixels_per_point;

        self.state
            .write()
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .state
            .read()
            .egui_ctx()
            .tessellate(full_output.shapes, pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .write()
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .write()
            .update_buffers(device, queue, encoder, &tris, screen_descriptor);

        let renderer = self.renderer.read();
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

        renderer.render(&mut render_pass, &tris, screen_descriptor);
        drop(render_pass);
        drop(renderer);
        for x in &full_output.textures_delta.free {
            self.renderer.write().free_texture(x);
        }
    }
}

pub struct EguiPlugin;

impl Plugin for EguiPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(begin_frame, SystemStage::PreUi)?;
        app.add_system(end_frame, SystemStage::PostUi)?;
        app.add_system(egui_event_hook, SystemStage::PostUi)?;
        app.add_system(crate::render, SystemStage::RenderUi)?;

        Ok(())
    }
    fn finish(&self, app: &mut App) -> Result<()> {
        let renderer = app.get_resource::<weaver_renderer::Renderer>().unwrap();
        let window = app.get_resource::<Window>().unwrap();
        let egui_context = EguiContext::new(renderer.device(), &window, 1);
        drop(renderer);
        drop(window);
        app.world().insert_resource(egui_context);

        Ok(())
    }
}

pub fn begin_frame(egui_context: Res<EguiContext>, window: Res<Window>) -> Result<()> {
    egui_context.begin_frame(&window);
    Ok(())
}

pub fn end_frame(egui_context: Res<EguiContext>) -> Result<()> {
    egui_context.end_frame();
    Ok(())
}

fn render(world: &Arc<World>) -> Result<()> {
    let Some(renderer) = world.get_resource::<weaver_renderer::Renderer>() else {
        return Ok(());
    };
    let Some(mut egui_context) = world.get_resource_mut::<EguiContext>() else {
        return Ok(());
    };
    let Some(window) = world.get_resource::<Window>() else {
        return Ok(());
    };
    let Some(current_frame) = renderer.current_frame() else {
        return Ok(());
    };
    let surface_texture_size = current_frame.surface_texture.texture.size();

    let screen_descriptor = ScreenDescriptor {
        pixels_per_point: 1.0,
        size_in_pixels: [surface_texture_size.width, surface_texture_size.height],
    };
    let mut encoder = renderer
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Egui command encoder"),
        });
    egui_context.render(
        renderer.device(),
        renderer.queue(),
        &mut encoder,
        &window,
        &current_frame.color_view,
        &screen_descriptor,
    );
    renderer.enqueue_command_buffer(encoder.finish());
    Ok(())
}

fn egui_event_hook(
    egui_context: Res<EguiContext>,
    window: Res<Window>,
    rx: EventRx<WinitEvent>,
) -> Result<()> {
    for event in rx.iter() {
        if let winit::event::Event::WindowEvent { window_id, event } = &event.event {
            if window.id() == *window_id {
                egui_context.handle_input(&window, event);
            }
        }
    }

    Ok(())
}
