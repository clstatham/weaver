use std::{
    any::TypeId,
    fmt::Debug,
    hash::Hash,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use asset::ExtractedRenderAssets;
use bind_group::{
    AssetBindGroupStaleness, BindGroupLayoutCache, ComponentBindGroupStaleness,
    ExtractedAssetBindGroups, ResourceBindGroupStaleness,
};
use camera::{CameraPlugin, ViewTarget};
use hdr::{HdrPlugin, HdrRenderTarget};
use mesh::MeshPlugin;
use pipeline::RenderPipelineCache;
use resources::ActiveCommandEncoder;
use texture::{
    texture_format::{DEPTH_FORMAT, VIEW_FORMAT},
    TexturePlugin,
};
use transform::TransformPlugin;
use weaver_app::{plugin::Plugin, App, AppLabel, SubApp};
use weaver_ecs::{
    commands::Commands,
    component::{Res, ResMut},
    entity::Entity,
    query::{Query, With},
    system::IntoSystemConfig,
    system_schedule::SystemStage,
    world::World,
    SystemStage,
};
use weaver_event::{EventRx, Events};
use weaver_util::prelude::*;
use weaver_winit::{Window, WindowResized, WindowSettings};

pub mod asset;
pub mod bind_group;
pub mod buffer;
pub mod camera;
pub mod clear_color;
pub mod extract;
pub mod hdr;
pub mod mesh;
pub mod pipeline;
pub mod resources;
pub mod shader;
pub mod texture;
pub mod transform;

pub mod prelude {
    pub use super::{
        bind_group::*,
        buffer::{GpuBuffer, GpuBufferVec},
        camera::{Camera, CameraPlugin, PrimaryCamera},
        clear_color::ClearColorPlugin,
        extract::ExtractComponent,
        pipeline::{
            ComputePipeline, ComputePipelineLayout, ComputePipelinePlugin, CreateComputePipeline,
            CreateRenderPipeline, RenderPipeline, RenderPipelineLayout, RenderPipelinePlugin,
        },
        Renderer, RendererPlugin, WgpuDevice, WgpuQueue,
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
            name: label.type_name(),
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

pub trait RenderLabel: Clone + Copy + 'static {
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub struct RenderApp;
impl AppLabel for RenderApp {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
pub enum RenderStage {
    Extract,
    ExtractBindGroup,
    ExtractPipeline,
    InitRenderResources,
    PreRender,
    Render,
    PostRender,
}

pub struct CurrentFrameInner {
    pub surface_texture: Arc<wgpu::SurfaceTexture>,
    pub color_view: Arc<wgpu::TextureView>,
    pub depth_view: Arc<wgpu::TextureView>,
}

#[derive(Default)]
pub struct CurrentFrame {
    pub inner: Option<CurrentFrameInner>,
}

pub struct WgpuInstance {
    pub instance: wgpu::Instance,
}

impl Deref for WgpuInstance {
    type Target = wgpu::Instance;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

pub struct WgpuAdapter {
    pub adapter: wgpu::Adapter,
}

impl Deref for WgpuAdapter {
    type Target = wgpu::Adapter;

    fn deref(&self) -> &Self::Target {
        &self.adapter
    }
}

pub struct WgpuDevice {
    pub device: wgpu::Device,
}

impl Deref for WgpuDevice {
    type Target = wgpu::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

pub struct WgpuQueue {
    pub queue: wgpu::Queue,
}

impl Deref for WgpuQueue {
    type Target = wgpu::Queue;

    fn deref(&self) -> &Self::Target {
        &self.queue
    }
}

pub struct WindowSurface {
    pub surface: wgpu::Surface<'static>,
}

impl Deref for WindowSurface {
    type Target = wgpu::Surface<'static>;

    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}

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
            memory_hints: wgpu::MemoryHints::Performance,
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

pub struct RenderAppChannels {
    pub main_to_render_tx: crossbeam_channel::Sender<SubApp>,
    pub render_to_main_rx: crossbeam_channel::Receiver<SubApp>,
}

#[derive(Default)]
pub struct ScratchMainWorld(World);

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

        render_app.world().init_resource::<CurrentFrame>();

        render_app
            .world_mut()
            .push_manual_stage(RenderStage::Extract);
        render_app
            .world_mut()
            .push_manual_stage(RenderStage::ExtractBindGroup);
        render_app
            .world_mut()
            .push_manual_stage(RenderStage::ExtractPipeline);

        render_app
            .world_mut()
            .init_resource::<ComponentBindGroupStaleness>();
        render_app
            .world_mut()
            .init_resource::<ResourceBindGroupStaleness>();
        render_app
            .world_mut()
            .init_resource::<AssetBindGroupStaleness>();

        render_app
            .world_mut()
            .push_manual_stage(RenderStage::InitRenderResources);
        render_app
            .world_mut()
            .push_manual_stage(RenderStage::PreRender);
        render_app
            .world_mut()
            .push_manual_stage(RenderStage::Render);
        render_app
            .world_mut()
            .push_manual_stage(RenderStage::PostRender);

        render_app
            .world()
            .insert_resource(RenderPipelineCache::new());
        render_app
            .world()
            .insert_resource(BindGroupLayoutCache::new());
        render_app
            .world()
            .insert_resource(ExtractedRenderAssets::new());
        render_app
            .world()
            .insert_resource(ExtractedAssetBindGroups::new());

        render_app
            .world_mut()
            .add_system(resize_surface, RenderStage::PreRender);
        render_app
            .world_mut()
            .add_system(begin_render.after(resize_surface), RenderStage::PreRender);
        render_app
            .world_mut()
            .add_system(end_render, RenderStage::PostRender);

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

    fn ready(&self, main_app: &App) -> bool {
        main_app.main_app().world().has_resource::<Window>()
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
            .world_mut()
            .get_resource_mut::<Events<WindowResized>>()
            .unwrap();

        let resized_events = resized_events.clone();

        let mut render_app = main_app.remove_sub_app::<RenderApp>().unwrap();

        render_app.world().insert_resource(WindowSettings {
            title: window.title(),
            width: window.inner_size().width,
            height: window.inner_size().height,
        });
        render_app.world().insert_resource(resized_events);
        create_surface(render_app.world_mut(), &window).unwrap();

        render_app.world().insert_resource(window);

        render_app.finish_plugins();
        render_app.world_mut().initialize_systems();

        render_to_main_tx.send(render_app).unwrap();

        main_app.insert_resource(RenderAppChannels {
            main_to_render_tx,
            render_to_main_rx,
        });

        tokio::spawn(async move {
            log::trace!("Entering render task main loop");

            loop {
                let Ok(mut render_app) = main_to_render_rx.recv() else {
                    break;
                };
                log::trace!("Received render app on render task");

                render_app.finish_plugins();

                log::trace!("Running render app stage: InitRenderResources");
                render_app
                    .world_mut()
                    .run_stage(RenderStage::InitRenderResources)
                    .await
                    .unwrap();
                log::trace!("Running render app stage: PreRender");
                render_app
                    .world_mut()
                    .run_stage(RenderStage::PreRender)
                    .await
                    .unwrap();
                log::trace!("Running render app stage: Render");
                render_app
                    .world_mut()
                    .run_stage(RenderStage::Render)
                    .await
                    .unwrap();
                log::trace!("Running render app stage: PostRender");
                render_app
                    .world_mut()
                    .run_stage(RenderStage::PostRender)
                    .await
                    .unwrap();

                log::trace!("Sending render app back to main task");

                if let Err(e) = render_to_main_tx.send(render_app) {
                    // we're probably shutting down
                    log::debug!("Failed to send render app back to main task: {}", e);
                    break;
                }
            }

            log::trace!("Exiting render task main loop");
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

pub async fn begin_render(
    commands: Commands,
    mut view_targets: Query<(Entity, &ViewTarget)>,
    device: Res<WgpuDevice>,
    mut renderer: ResMut<Renderer>,
    surface: Res<WindowSurface>,
    mut current_frame: ResMut<CurrentFrame>,
) {
    if current_frame.inner.is_some() {
        return;
    }

    let view_targets = view_targets
        .iter()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    for entity in view_targets {
        commands.remove_component::<ViewTarget>(entity).await;
    }

    let frame = match surface.get_current_texture() {
        Ok(frame) => frame,
        Err(e) => {
            // TODO: FIXME: This could happen when the window is moved to a different monitor
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

    let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    commands
        .insert_resource(ActiveCommandEncoder::new(encoder))
        .await;
}

pub async fn end_render(
    commands: Commands,
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

    if let Some(encoder) = commands.remove_resource::<ActiveCommandEncoder>().await {
        renderer.enqueue_command_buffer(encoder.finish());
    } else {
        log::warn!("No active command encoder to end render");
    }

    let command_buffers = std::mem::take(&mut renderer.command_buffers);

    queue.submit(command_buffers);

    let surface_texture = Arc::into_inner(surface_texture).unwrap();

    log::trace!("Presenting frame");
    surface_texture.present();
    log::trace!("Frame presented");
}

#[allow(clippy::too_many_arguments)]
async fn resize_surface(
    commands: Commands,
    events: EventRx<WindowResized>,
    mut window_size: ResMut<WindowSettings>,
    mut view_targets: Query<(Entity, With<ViewTarget>)>,
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
        let view_targets = view_targets
            .iter()
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>();
        if current_frame.inner.take().is_some() {
            has_current_frame = true;

            for entity in view_targets {
                commands.remove_component::<ViewTarget>(entity).await;
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
                commands
                    .insert_component(view_target_entity, view_target.clone())
                    .await;
            }
        }

        hdr_target.resize(&device, width, height);
    }
}
