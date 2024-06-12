use std::sync::Arc;

use asset::ExtractedRenderAssets;
use bind_group::ExtractedAssetBindGroups;
use camera::CameraPlugin;
use mesh::MeshPlugin;
use texture::TexturePlugin;
use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{
    prelude::Resource,
    system::{Res, SystemStage},
};
use weaver_util::lock::Lock;
use weaver_winit::Window;

pub mod asset;
pub mod bind_group;
pub mod buffer;
pub mod camera;
pub mod clear_color;
pub mod extract;
pub mod graph;
pub mod mesh;
pub mod shader;
pub mod texture;
pub mod transform;

pub mod prelude {
    pub use super::camera::{Camera, CameraPlugin};
    pub use super::extract::RenderComponent;
    pub use super::graph::{Render, RenderGraph};
    pub use super::{Renderer, RendererPlugin};
    pub use wgpu;
}

pub struct CurrentFrame {
    pub surface_texture: wgpu::SurfaceTexture,
    pub color_view: Arc<wgpu::TextureView>,
    pub depth_view: Arc<wgpu::TextureView>,
}

#[derive(Resource)]
pub struct Renderer {
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    window_surface: Option<wgpu::Surface<'static>>,
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    current_frame: Lock<Option<CurrentFrame>>,
    depth_texture: Lock<Option<Arc<wgpu::Texture>>>,
    command_buffers: Lock<Vec<wgpu::CommandBuffer>>,
}

impl Renderer {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            instance: None,
            adapter: None,
            window_surface: None,
            device: None,
            queue: None,
            current_frame: Lock::new(None),
            depth_texture: Lock::new(None),
            command_buffers: Lock::new(Vec::new()),
        }
    }

    pub fn device(&self) -> &Arc<wgpu::Device> {
        self.device.as_ref().unwrap()
    }

    pub fn queue(&self) -> &Arc<wgpu::Queue> {
        self.queue.as_ref().unwrap()
    }

    pub fn current_frame_view(&self) -> Option<(Arc<wgpu::TextureView>, Arc<wgpu::TextureView>)> {
        self.current_frame
            .read()
            .as_ref()
            .map(|frame| (frame.color_view.clone(), frame.depth_view.clone()))
    }

    pub fn create_surface(&mut self, window: &Window) -> anyhow::Result<()> {
        if self.window_surface.is_some() {
            log::warn!("Surface already created");
            return Ok(());
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(&**window).unwrap())?
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits:
                    wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                label: None,
            },
            None,
        ))
        .unwrap();

        let caps = surface.get_capabilities(&adapter);

        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
                format: wgpu::TextureFormat::Bgra8Unorm,
                width: window.inner_size().width,
                height: window.inner_size().height,
                present_mode: wgpu::PresentMode::AutoNoVsync,
                desired_maximum_frame_latency: 1,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
            },
        );

        self.window_surface = Some(surface);

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: window.inner_size().width,
                height: window.inner_size().height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        *self.depth_texture.write() = Some(Arc::new(depth_texture));

        self.instance = Some(instance);
        self.adapter = Some(adapter);
        self.device = Some(Arc::new(device));
        self.queue = Some(Arc::new(queue));

        Ok(())
    }

    pub fn begin_frame(&self) -> anyhow::Result<()> {
        if self.current_frame.read().is_some() {
            log::warn!("Current frame already exists");
            return Ok(());
        }

        log::trace!("Begin frame");

        let surface = self.window_surface.as_ref().unwrap();
        let frame = surface.get_current_texture()?;
        let color_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view =
            self.depth_texture
                .read()
                .as_ref()
                .unwrap()
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Depth Texture View"),
                    format: Some(wgpu::TextureFormat::Depth32Float),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    ..Default::default()
                });
        *self.current_frame.write() = Some(CurrentFrame {
            surface_texture: frame,
            color_view: Arc::new(color_view),
            depth_view: Arc::new(depth_view),
        });

        Ok(())
    }

    pub fn enqueue_command_buffer(&self, command_buffer: wgpu::CommandBuffer) {
        self.command_buffers.write().push(command_buffer);
    }

    pub fn end_frame(&self) -> anyhow::Result<()> {
        if self.current_frame.read().is_none() {
            log::warn!("No current frame to end");
            return Ok(());
        }

        log::trace!("End frame");

        let CurrentFrame {
            surface_texture, ..
        } = self.current_frame.write().take().unwrap();

        let command_buffers = self.command_buffers.write().drain(..).collect::<Vec<_>>();
        self.queue().submit(command_buffers);

        surface_texture.present();

        Ok(())
    }
}

pub struct RendererPlugin;

impl Plugin for RendererPlugin {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        app.world().insert_resource(Renderer::new());
        app.world().insert_resource(ExtractedRenderAssets::new());
        app.world().insert_resource(ExtractedAssetBindGroups::new());

        app.add_plugin(CameraPlugin)?;
        // app.add_plugin(TransformPlugin)?;
        app.add_plugin(MeshPlugin)?;
        app.add_plugin(TexturePlugin)?;

        app.add_system(begin_render, SystemStage::PreRender)?;
        app.add_system(end_render, SystemStage::PostRender)?;

        Ok(())
    }

    fn finish(&self, app: &mut App) -> anyhow::Result<()> {
        let mut renderer = app.world().get_resource_mut::<Renderer>().unwrap();
        let window = app.world().get_resource::<Window>().unwrap();
        renderer.create_surface(&window)?;

        Ok(())
    }
}

fn begin_render(renderer: Res<Renderer>) -> anyhow::Result<()> {
    renderer.begin_frame()?;

    Ok(())
}

fn end_render(renderer: Res<Renderer>) -> anyhow::Result<()> {
    renderer.end_frame()?;

    Ok(())
}
