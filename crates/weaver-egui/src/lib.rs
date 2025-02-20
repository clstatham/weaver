use egui::{Context, FullOutput};
use egui_wgpu::{Renderer, ScreenDescriptor};
use egui_winit::{State, winit};
use weaver_app::{App, AppStage, plugin::Plugin};
use weaver_ecs::{
    SystemStage,
    component::{Res, ResMut},
    prelude::Commands,
    system_schedule::SystemStage,
};
use weaver_event::EventRx;
use weaver_renderer::{
    CurrentFrame, MainWorld, RenderApp, RenderStage, WgpuDevice, WgpuQueue, prelude::wgpu,
    texture::texture_format,
};
use weaver_util::prelude::*;
use weaver_winit::{Window, WinitEvent};

pub use egui;

pub mod prelude {
    pub use super::{EguiContext, EguiPlugin};
    pub use egui;
}

#[derive(Clone)]
pub struct EguiContext {
    state: SharedLock<State>,
    renderer: SharedLock<Renderer>,
    full_output: SharedLock<Option<FullOutput>>,
}

impl EguiContext {
    pub fn new(device: &wgpu::Device, window: &winit::window::Window) -> Self {
        let ctx = Context::default();
        let viewport_id = ctx.viewport_id();
        let state = SharedLock::new(State::new(ctx, viewport_id, window, None, None, None));
        let renderer = SharedLock::new(Renderer::new(
            device,
            texture_format::VIEW_FORMAT,
            None,
            1,
            false,
        ));

        Self {
            state,
            renderer,
            full_output: SharedLock::new(None),
        }
    }

    pub fn available_rect(&self) -> egui::Rect {
        self.state.read().egui_ctx().available_rect()
    }

    pub fn handle_input(&self, window: &winit::window::Window, event: &winit::event::WindowEvent) {
        let _ = self.state.write().on_window_event(window, event);
    }

    pub fn wants_input(&self) -> bool {
        self.state.read().egui_ctx().wants_keyboard_input()
            || self.state.read().egui_ctx().wants_pointer_input()
    }

    pub fn begin_frame(&self, window: &winit::window::Window) {
        let raw_input = self.state.write().take_egui_input(window);
        self.state.read().egui_ctx().begin_pass(raw_input);
    }

    pub fn end_frame(&self) {
        *self.full_output.write() = Some(self.state.read().egui_ctx().end_pass());
    }

    pub fn with_ctx<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&egui::Context) -> R,
    {
        f(self.state.read().egui_ctx())
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

    pub fn pre_render_on_main_thread(&self, window: &winit::window::Window) {
        if self.full_output.read().is_none() {
            return;
        }
        let full_output = self.full_output.read();
        let full_output = full_output.as_ref().unwrap();

        self.state
            .write()
            .handle_platform_output(window, full_output.platform_output.clone());
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window_surface_view: &wgpu::TextureView,
        screen_descriptor: &ScreenDescriptor,
    ) {
        if self.full_output.read().is_none() {
            return;
        }
        let pixels_per_point = screen_descriptor.pixels_per_point;

        let full_output = self.full_output.read();
        let full_output = full_output.as_ref().unwrap().clone();

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
        let mut render_pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
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
            })
            .forget_lifetime();

        renderer.render(&mut render_pass, &tris, screen_descriptor);
        drop(render_pass);
        drop(renderer);
        for x in &full_output.textures_delta.free {
            self.renderer.write().free_texture(x);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
pub struct RenderUi;

pub struct EguiPlugin;

impl Plugin for EguiPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(begin_frame, AppStage::PreUpdate);
        app.add_system(end_frame, AppStage::PostUpdate);
        app.add_system(egui_events, AppStage::PostUpdate);
        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app
            .world_mut()
            .add_system(extract_egui_context, RenderStage::Extract);
        render_app
            .world_mut()
            .add_update_stage_after(RenderUi, RenderStage::Render);
        render_app.world_mut().add_system(render, RenderUi);

        Ok(())
    }
}

async fn extract_egui_context(
    commands: Commands,
    main_world: Res<MainWorld>,
    window: Res<Window>,
    device: Res<WgpuDevice>,
) {
    if commands.has_resource::<EguiContext>() {
        return;
    }
    let egui_context = EguiContext::new(&device, &window);
    commands.insert_resource(egui_context.clone());
    main_world.insert_resource(egui_context);
}

pub async fn begin_frame(egui_context: Res<EguiContext>, window: Res<Window>) {
    egui_context.begin_frame(&window);
}

pub async fn end_frame(egui_context: Res<EguiContext>, window: Res<Window>) {
    egui_context.end_frame();

    egui_context.pre_render_on_main_thread(&window);
}

async fn render(
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
    current_frame: Res<CurrentFrame>,
    mut egui_context: ResMut<EguiContext>,
    mut renderer: ResMut<weaver_renderer::Renderer>,
) {
    let Some(current_frame) = current_frame.inner.as_ref() else {
        return;
    };
    let surface_texture_size = current_frame.surface_texture.texture.size();

    let screen_descriptor = ScreenDescriptor {
        pixels_per_point: 1.0,
        size_in_pixels: [surface_texture_size.width, surface_texture_size.height],
    };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Egui command encoder"),
    });
    egui_context.render(
        &device,
        &queue,
        &mut encoder,
        &current_frame.color_view,
        &screen_descriptor,
    );

    renderer.enqueue_command_buffer(encoder.finish());
}

async fn egui_events(egui_context: Res<EguiContext>, window: Res<Window>, rx: EventRx<WinitEvent>) {
    for event in rx.iter() {
        if let winit::event::Event::WindowEvent { window_id, event } = &event.event {
            if window.id() == *window_id {
                egui_context.handle_input(&window, event);
            }
        }
    }
}
