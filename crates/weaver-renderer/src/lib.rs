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
use hdr::{HdrPlugin, HdrRenderTarget};
use mesh::MeshPlugin;
use pipeline::RenderPipelineCache;
use texture::{
    texture_format::{DEPTH_FORMAT, VIEW_FORMAT},
    TexturePlugin,
};
use transform::TransformPlugin;
use weaver_app::{plugin::Plugin, App, AppLabel, SubApp};
use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    prelude::Resource,
    query::Query,
    reflect::registry::TypeRegistry,
    system_schedule::SystemStage,
    world::World,
};
use weaver_event::{EventRx, Events};
use weaver_util::Result;
use weaver_winit::{Window, WindowResized, WindowSize};

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
        extract::ExtractComponent,
        graph::{RenderGraph, RenderNode, Slot},
        pipeline::{
            ComputePipeline, ComputePipelineLayout, ComputePipelinePlugin, CreateComputePipeline,
            CreateRenderPipeline, RenderPipeline, RenderPipelineLayout, RenderPipelinePlugin,
        },
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

pub struct ExtractStage;
impl SystemStage for ExtractStage {}

pub struct ExtractBindGroupStage;
impl SystemStage for ExtractBindGroupStage {}

pub struct ExtractPipelineStage;
impl SystemStage for ExtractPipelineStage {}

pub struct InitRenderResources;
impl SystemStage for InitRenderResources {}

pub struct PreRender;
impl SystemStage for PreRender {}

pub struct Render;
impl SystemStage for Render {}

pub struct PostRender;
impl SystemStage for PostRender {}

pub struct CurrentFrameInner {
    pub surface_texture: Arc<wgpu::SurfaceTexture>,
    pub color_view: Arc<wgpu::TextureView>,
    pub depth_view: Arc<wgpu::TextureView>,
}

#[derive(Resource, Default)]
pub struct CurrentFrame {
    pub inner: Option<CurrentFrameInner>,
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

fn create_surface(render_world: &mut World, window: &Window) -> Result<()> {
    if render_world.has_resource::<WindowSurface>() {
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

    let mut required_limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
    required_limits.max_push_constant_size = 256;

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            required_features: wgpu::Features::MULTIVIEW
                | wgpu::Features::PUSH_CONSTANTS
                | wgpu::Features::TEXTURE_BINDING_ARRAY
                | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
            required_limits,
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

    pub fn enqueue_command_buffers(
        &mut self,
        command_buffers: impl IntoIterator<Item = wgpu::CommandBuffer>,
    ) {
        self.command_buffers.extend(command_buffers);
    }
}

pub struct RenderExtractApp;
impl AppLabel for RenderExtractApp {}

#[derive(Resource)]
pub struct RenderAppChannels {
    pub main_to_render_tx: crossbeam_channel::Sender<SubApp>,
    pub render_to_main_rx: crossbeam_channel::Receiver<SubApp>,
}

#[derive(Resource, Default)]
pub struct ScratchMainWorld(World);

#[derive(Resource)]
pub struct MainWorld(World);

impl Deref for MainWorld {
    type Target = World;

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

        render_app.init_resource::<CurrentFrame>();

        render_app.push_manual_stage::<ExtractStage>();
        render_app.push_manual_stage::<ExtractBindGroupStage>();
        render_app.push_manual_stage::<ExtractPipelineStage>();

        render_app.push_update_stage::<InitRenderResources>();
        render_app.push_update_stage::<PreRender>();
        render_app.push_update_stage::<Render>();
        render_app.push_update_stage::<PostRender>();

        render_app.insert_resource(RenderGraph::new());

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

        let mut extract_app = SubApp::new();
        extract_app.set_extract(Box::new(renderer_extract));
        main_app.add_sub_app::<RenderExtractApp>(extract_app);

        Ok(())
    }

    fn finish(&self, main_app: &mut App) -> Result<()> {
        let (main_to_render_tx, main_to_render_rx) = crossbeam_channel::bounded(1);
        let (render_to_main_tx, render_to_main_rx) = crossbeam_channel::bounded(1);

        let window = main_app
            .main_app()
            .world()
            .get_resource::<Window>()
            .unwrap()
            .clone();
        let resized_events = main_app
            .main_app_mut()
            .get_resource_mut::<Events<WindowResized>>()
            .unwrap()
            .into_inner();

        let resized_events = resized_events.clone();

        let mut render_app: SubApp = main_app.remove_sub_app::<RenderApp>().unwrap();

        render_app.insert_resource(WindowSize {
            width: window.inner_size().width,
            height: window.inner_size().height,
        });
        render_app.insert_resource(resized_events);
        create_surface(render_app.world_mut(), &window).unwrap();

        render_app.insert_resource(window);

        render_app.finish_plugins();

        render_to_main_tx.send(render_app).unwrap();

        main_app.insert_resource(RenderAppChannels {
            main_to_render_tx,
            render_to_main_rx,
        });

        std::thread::spawn(move || {
            log::trace!("Entering render thread main loop");

            loop {
                let Ok(mut render_app) = main_to_render_rx.recv() else {
                    break;
                };
                log::trace!("Received render app on render thread");

                render_app.update();

                log::trace!("Sending render app back to main thread");

                render_to_main_tx.send(render_app).unwrap();
            }

            log::trace!("Exiting render thread main loop");
        });

        Ok(())
    }
}

fn renderer_extract(main_world: &mut World, _world: &mut World) -> Result<()> {
    let channels = main_world.remove_resource::<RenderAppChannels>().unwrap();
    let mut render_app = channels.render_to_main_rx.recv().unwrap();
    log::trace!("Received render app on main thread");
    render_app.extract_from(main_world).unwrap();
    log::trace!("Sending render app back to render thread");
    channels.main_to_render_tx.send(render_app).unwrap();
    main_world.insert_resource(channels);

    Ok(())
}

pub fn begin_render(
    world: &mut World,
    device: Res<WgpuDevice>,
    mut renderer: ResMut<Renderer>,
    surface: Res<WindowSurface>,
    mut current_frame: ResMut<CurrentFrame>,
) {
    if current_frame.inner.is_some() {
        return;
    }

    let view_targets = world.query::<&ViewTarget>();
    let view_targets = view_targets.entity_iter(world).collect::<Vec<_>>();
    for entity in view_targets {
        world.remove_component::<ViewTarget>(entity);
    }

    let frame = match surface.get_current_texture() {
        Ok(frame) => frame,
        Err(e) => {
            panic!("Failed to acquire next surface texture: {}", e);
        }
    };

    log::trace!("Begin render");

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

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Initial Encoder"),
    });
    {
        let mut _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Initial Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
    }
    renderer.enqueue_command_buffer(encoder.finish());

    current_frame.inner.replace(CurrentFrameInner {
        surface_texture: Arc::new(frame),
        color_view: Arc::new(color_view),
        depth_view: Arc::new(depth_view),
    });
}

pub fn end_render(
    mut current_frame: ResMut<CurrentFrame>,
    mut renderer: ResMut<Renderer>,
    queue: Res<WgpuQueue>,
) {
    let Some(current_frame) = current_frame.inner.take() else {
        log::warn!("No current frame to end");
        return;
    };

    log::trace!("End render");

    let CurrentFrameInner {
        surface_texture, ..
    } = current_frame;

    let command_buffers = std::mem::take(&mut renderer.command_buffers);

    queue.submit(command_buffers);

    let surface_texture = Arc::into_inner(surface_texture).unwrap();

    log::trace!("Presenting frame");
    surface_texture.present();
    log::trace!("Frame presented");
}

#[allow(clippy::too_many_arguments)]
fn resize_surface(
    mut commands: Commands,
    events: EventRx<WindowResized>,
    mut window_size: ResMut<WindowSize>,
    view_targets: Query<&ViewTarget>,
    mut current_frame: ResMut<CurrentFrame>,
    mut renderer: ResMut<Renderer>,
    device: Res<WgpuDevice>,
    surface: Res<WindowSurface>,
    mut hdr_target: ResMut<HdrRenderTarget>,
) {
    let mut events_vec = events.iter().collect::<Vec<_>>();
    if let Some(event) = events_vec.pop() {
        let mut has_current_frame = false;
        let mut view_target_entities = Vec::new();
        let view_targets = view_targets.entity_iter().collect::<Vec<_>>();
        if current_frame.inner.take().is_some() {
            has_current_frame = true;

            for entity in view_targets {
                commands.remove_component::<ViewTarget>(entity);
                view_target_entities.push(entity);
            }
        }

        let WindowResized { width, height } = *event;

        log::trace!("Resizing surface to {}x{}", width, height);

        window_size.width = width;
        window_size.height = height;

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

        if has_current_frame {
            let surface_texture = surface.get_current_texture().unwrap();
            let color_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let depth_view = renderer.depth_texture.as_ref().unwrap().create_view(
                &wgpu::TextureViewDescriptor {
                    label: Some("Depth Texture View"),
                    format: Some(DEPTH_FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    ..Default::default()
                },
            );

            let current_frame_inner = CurrentFrameInner {
                surface_texture: Arc::new(surface_texture),
                color_view: Arc::new(color_view),
                depth_view: Arc::new(depth_view),
            };

            current_frame.inner.replace(current_frame_inner);
            let view_target = ViewTarget::from((&*current_frame, &*hdr_target));
            for view_target_entity in view_target_entities {
                commands.insert_component(view_target_entity, view_target.clone());
            }
        }

        hdr_target.resize(&device, width, height);
    }
}

pub fn render_system(render_world: &mut World) {
    let view_targets = render_world.query::<&ViewTarget>();
    let view_targets = view_targets.entity_iter(render_world).collect::<Vec<_>>();
    let mut render_graph = render_world.remove_resource::<RenderGraph>().unwrap();
    render_graph.prepare(render_world).unwrap();

    let mut renderer = render_world.remove_resource::<Renderer>().unwrap();

    let device = render_world.get_resource::<WgpuDevice>().unwrap();
    let queue = render_world.get_resource::<WgpuQueue>().unwrap();

    // todo: don't assume every camera wants to run the whole main render graph
    for entity in view_targets {
        log::trace!("Running render graph for entity: {:?}", entity);
        render_graph
            .run(&device, &queue, &mut renderer, render_world, entity)
            .unwrap();
    }

    drop((device, queue));

    render_world.insert_resource(renderer);

    render_world.insert_resource(render_graph);
}
