use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::Hash,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use asset::ExtractedRenderAssets;
use bind_group::{BindGroupLayoutCache, ExtractedAssetBindGroups};
use camera::{CameraPlugin, ViewTarget};
use graph::RenderGraph;
use hdr::HdrPlugin;
use mesh::MeshPlugin;
use pipeline::RenderPipelineCache;
use texture::{
    texture_format::{DEPTH_FORMAT, VIEW_FORMAT},
    TexturePlugin,
};
use transform::TransformPlugin;
use weaver_app::{plugin::Plugin, App, AppLabel, SubApp};
use weaver_asset::Assets;
use weaver_ecs::{
    component::{Res, ResMut},
    prelude::Resource,
    query::Query,
    reflect::registry::TypeRegistry,
    system_schedule::SystemStage,
    world::{ReadWorld, World, WorldLock, WriteWorld},
};
use weaver_event::{EventRx, Events};
use weaver_util::prelude::Result;
use weaver_winit::{Window, WindowResized};

pub mod asset;
pub mod bind_group;
pub mod buffer;
pub mod camera;
pub mod clear_color;
pub mod draw_fn;
pub mod extract;
pub mod graph;
pub mod hdr;
pub mod mesh;
pub mod pipeline;
pub mod render_command;
pub mod render_phase;
pub mod shader;
pub mod texture;
pub mod transform;

pub mod prelude {
    pub use super::{
        camera::{Camera, CameraPlugin},
        draw_fn::{DrawFn, DrawFnsApp, DrawFunctions},
        extract::RenderComponent,
        graph::{RenderGraph, RenderNode, Slot},
        Renderer, RendererPlugin,
    };
    pub use encase;
    pub use wgpu;
}

#[derive(Clone, Copy)]
pub struct RenderId {
    pub id: TypeId,
    pub name: &'static str,
}

impl RenderId {
    pub fn of<T: RenderLabel>(label: T) -> Self {
        Self {
            id: TypeId::of::<T>(),
            name: label.name(),
        }
    }
}

impl Debug for RenderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RenderId").field(&self.name).finish()
    }
}

impl PartialEq for RenderId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for RenderId {}

impl Hash for RenderId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

pub trait RenderLabel: Any + Clone + Copy {
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub struct RenderApp;
impl AppLabel for RenderApp {}

pub struct Extract;
impl SystemStage for Extract {}

pub struct ExtractBindGroups;
impl SystemStage for ExtractBindGroups {}

pub struct PreRender;
impl SystemStage for PreRender {}

pub struct Render;
impl SystemStage for Render {}

pub struct PostRender;
impl SystemStage for PostRender {}

#[derive(Resource)]
pub struct CurrentFrame {
    pub surface_texture: Arc<wgpu::SurfaceTexture>,
    pub color_view: Arc<wgpu::TextureView>,
    pub depth_view: Arc<wgpu::TextureView>,
}

#[derive(Resource)]
pub struct WgpuInstance {
    pub instance: wgpu::Instance,
}

impl Deref for WgpuInstance {
    type Target = wgpu::Instance;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

#[derive(Resource)]
pub struct WgpuAdapter {
    pub adapter: wgpu::Adapter,
}

impl Deref for WgpuAdapter {
    type Target = wgpu::Adapter;

    fn deref(&self) -> &Self::Target {
        &self.adapter
    }
}

#[derive(Resource)]
pub struct WgpuDevice {
    pub device: wgpu::Device,
}

impl Deref for WgpuDevice {
    type Target = wgpu::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

#[derive(Resource)]
pub struct WgpuQueue {
    pub queue: wgpu::Queue,
}

impl Deref for WgpuQueue {
    type Target = wgpu::Queue;

    fn deref(&self) -> &Self::Target {
        &self.queue
    }
}

#[derive(Resource)]
pub struct WindowSurface {
    pub surface: wgpu::Surface<'static>,
}

impl Deref for WindowSurface {
    type Target = wgpu::Surface<'static>;

    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}

#[derive(Resource)]
pub struct Renderer {
    depth_texture: Option<Arc<wgpu::Texture>>,
    command_buffers: Vec<wgpu::CommandBuffer>,
}

fn create_surface(render_world: &mut World) -> Result<()> {
    if render_world.get_resource::<WindowSurface>().is_some() {
        log::warn!("Surface already created");
        return Ok(());
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let window = render_world.get_resource::<Window>().unwrap();

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
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
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
            format: VIEW_FORMAT,
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            desired_maximum_frame_latency: 1,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        },
    );

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
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    render_world.insert_resource(WgpuInstance { instance });
    render_world.insert_resource(WgpuAdapter { adapter });
    render_world.insert_resource(WgpuDevice { device });
    render_world.insert_resource(WgpuQueue { queue });
    render_world.insert_resource(WindowSurface { surface });
    render_world.insert_resource(Renderer {
        depth_texture: Some(Arc::new(depth_texture)),
        command_buffers: Vec::new(),
    });

    Ok(())
}

impl Renderer {
    pub fn enqueue_command_buffer(&mut self, command_buffer: wgpu::CommandBuffer) {
        self.command_buffers.push(command_buffer);
    }
}

#[derive(Resource, Default)]
pub struct ScratchMainWorld(WorldLock);

#[derive(Resource)]
pub struct MainWorld(WorldLock);

impl Deref for MainWorld {
    type Target = WorldLock;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MainWorld {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct RendererPlugin;

impl Plugin for RendererPlugin {
    fn build(&self, main_app: &mut App) -> Result<()> {
        main_app.insert_resource(ScratchMainWorld::default());

        let mut render_app = SubApp::new();

        render_app.insert_resource(TypeRegistry::new());
        render_app.insert_resource(Assets::new());

        render_app.push_manual_stage::<Extract>();
        render_app.push_manual_stage::<ExtractBindGroups>();

        render_app.push_update_stage::<PreRender>();
        render_app.push_update_stage::<Render>();
        render_app.push_update_stage::<PostRender>();

        let mut render_graph = RenderGraph::new();
        render_graph.set_inputs(vec![]).unwrap();
        render_app.insert_resource(render_graph);

        render_app.insert_resource(RenderPipelineCache::new());
        render_app.insert_resource(BindGroupLayoutCache::new());
        render_app.insert_resource(ExtractedRenderAssets::new());
        render_app.insert_resource(ExtractedAssetBindGroups::new());

        render_app.add_system(resize_surface, PreRender);
        render_app.add_system_after(begin_render, resize_surface, PreRender);
        render_app.add_system(render_system, Render);
        render_app.add_system(end_render, PostRender);

        render_app.add_plugin(CameraPlugin)?;
        render_app.add_plugin(TransformPlugin)?;
        render_app.add_plugin(MeshPlugin)?;
        render_app.add_plugin(TexturePlugin)?;
        render_app.add_plugin(HdrPlugin)?;

        render_app.set_extract(Box::new(extract::render_extract));

        main_app.add_sub_app::<RenderApp>(render_app);

        Ok(())
    }

    fn finish(&self, main_app: &mut App) -> Result<()> {
        let window = main_app.main_app().get_resource::<Window>().unwrap();
        let resized_events = main_app
            .main_app()
            .get_resource::<Events<WindowResized>>()
            .unwrap();
        let render_app = main_app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.insert_resource(resized_events.clone());
        render_app.insert_resource(window.clone());
        create_surface(&mut render_app.write_world())?;

        Ok(())
    }
}

pub fn begin_render(mut render_world: WriteWorld) -> Result<()> {
    let renderer = render_world.get_resource::<Renderer>().unwrap();
    if render_world.has_resource::<CurrentFrame>() {
        log::warn!("Current frame already exists");
        return Ok(());
    }

    log::trace!("Begin frame");

    let surface = render_world.get_resource::<WindowSurface>().unwrap();
    let frame = surface.get_current_texture().unwrap();
    let color_view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view =
        renderer
            .depth_texture
            .as_ref()
            .unwrap()
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Depth Texture View"),
                format: Some(DEPTH_FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2),
                ..Default::default()
            });
    let current_frame = CurrentFrame {
        surface_texture: Arc::new(frame),
        color_view: Arc::new(color_view),
        depth_view: Arc::new(depth_view),
    };

    render_world.insert_resource(current_frame);

    Ok(())
}

pub fn end_render(mut render_world: WriteWorld) -> Result<()> {
    let Some(current_frame) = render_world.remove_resource::<CurrentFrame>() else {
        return Ok(());
    };

    let mut renderer = render_world.get_resource_mut::<Renderer>().unwrap();
    let device = render_world.get_resource::<WgpuDevice>().unwrap();
    let queue = render_world.get_resource::<WgpuQueue>().unwrap();

    log::trace!("End frame");

    let CurrentFrame {
        surface_texture,
        color_view,
        depth_view,
    } = current_frame;

    let mut command_buffers = renderer.command_buffers.drain(..).collect::<Vec<_>>();

    if command_buffers.is_empty() {
        // ensure that we have at least some work to do
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Final Encoder"),
        });
        {
            let mut _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Final Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
        }
        command_buffers.push(encoder.finish());
    }

    queue.submit(command_buffers);

    let surface_texture = Arc::into_inner(surface_texture).unwrap();

    surface_texture.present();

    Ok(())
}

fn resize_surface(render_world: ReadWorld, events: EventRx<WindowResized>) -> Result<()> {
    for event in events.iter() {
        // if multiple events are queued up, only resize the window to the last event's size
        let WindowResized { width, height } = *event;

        let mut renderer = render_world.get_resource_mut::<Renderer>().unwrap();
        let device = render_world.get_resource::<WgpuDevice>().unwrap();
        let surface = render_world.get_resource::<WindowSurface>().unwrap();

        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
                format: VIEW_FORMAT,
                width,
                height,
                present_mode: wgpu::PresentMode::AutoNoVsync,
                desired_maximum_frame_latency: 1,
                alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: vec![],
            },
        );

        let depth_texture = renderer.depth_texture.take().unwrap();

        depth_texture.destroy();

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        renderer.depth_texture = Some(Arc::new(depth_texture));
    }
    Ok(())
}

pub fn render_system(
    render_world: WorldLock,
    mut render_graph: ResMut<RenderGraph>,
    mut renderer: ResMut<Renderer>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
    view_targets: Query<&ViewTarget>,
) -> Result<()> {
    render_graph.prepare(&render_world).unwrap();

    // todo: don't assume every camera wants to run the whole main render graph
    for entity in view_targets.entity_iter() {
        render_graph
            .run(&device, &queue, &mut renderer, &render_world, entity)
            .unwrap();
    }

    Ok(())
}

#[doc(hidden)]
#[allow(unused)]
mod hidden {
    use super::*;
    use weaver_ecs::system::assert_is_non_exclusive_system;

    fn system_assertions() {
        // uncomment these lines periodically to check make sure the assertions fail
        // (they should cause compiler errors)

        // assert_is_non_exclusive_system(begin_render);
        // assert_is_non_exclusive_system(end_render);
        // assert_is_non_exclusive_system(render_system);
    }
}
