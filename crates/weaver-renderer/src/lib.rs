use std::ops::{Deref, DerefMut};

use bind_group::BindGroupLayouts;
use camera::CameraPlugin;
use extract::Extract;
use mesh::MeshPlugin;
use phase::{Background3d, Opaque3d, Overlay3d, RenderPhasePlugin};
use shader::ShaderLibrary;
use surface::{acquire_frame, frame_in_flight, present_frame, resize_surface};
use sync::{RenderEntities, despawn_orphaned_render_entities};
use texture::TexturePlugin;
use tonemap::TonemapPlugin;
use transform::TransformPlugin;
use view::ViewPlugin;
use weaver_app::{App, AppLabel, SubApp, plugin::Plugin};
use weaver_ecs::{
    SystemStage, system::IntoSystemConfig, system_schedule::SystemStage, world::World,
};
use weaver_event::{EventQueue, EventRx, EventTx};
use weaver_util::prelude::*;
use weaver_winit::{Window, WindowResized};

// Identifies one GPU allocation (a buffer or texture). A new id is minted whenever a resource is
// (re)created, so a bind group can tell that a resource it references was replaced by comparing
// ids, instead of relying on someone remembering to mark it stale.
define_atomic_id!(GpuResourceId);

pub mod asset;
pub mod bind_group;
pub mod buffer;
pub mod camera;
pub mod clear_color;
pub mod driver;
pub mod extract;
pub mod mesh;
pub mod phase;
pub mod pipeline;
pub mod resources;
pub mod shader;
pub mod surface;
pub mod sync;
pub mod texture;
pub mod tonemap;
pub mod tracked_pass;
pub mod transform;
pub mod view;

pub mod prelude {
    pub use super::{
        RendererPlugin, WgpuDevice, WgpuQueue,
        bind_group::*,
        buffer::{GpuBuffer, GpuBufferVec},
        camera::{Camera, CameraPlugin, PrimaryCamera},
        clear_color::ClearColorPlugin,
        extract::ExtractComponent,
        phase::{
            AddDrawFunction, Background3d, DrawFunction, DrawFunctions, Opaque3d, Opaque3dKey,
            Overlay3d, RenderPhase,
        },
        pipeline::{
            ComputePipeline, DepthTest, InitRenderResourcePlugin, RenderPipeline,
            RenderPipelineDescriptor,
        },
        shader::ShaderLibrary,
        surface::{SurfaceFrame, frame_in_flight},
        tracked_pass::TrackedRenderPass,
        view::{ExtractedView, ViewTarget},
    };
    pub use encase;
    pub use wgpu;
}

pub struct RenderApp;
impl AppLabel for RenderApp {}

/// The render world's stages, in the order they run each frame.
///
/// `Extract` runs on the main thread while the main world is borrowed, so it should only copy
/// data. Everything after it runs on the render thread, overlapping the main world's next frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemStage)]
pub enum RenderStage {
    /// Copy what the frame needs out of the main world. No GPU work.
    Extract,
    /// Create or update GPU versions of assets (meshes, textures, materials) and pipelines.
    PrepareAssets,
    /// Per-frame GPU data: uniforms, light buffers, render targets. Also resizes the surface and
    /// acquires the frame's swapchain texture.
    Prepare,
    /// Decide what each view draws, by adding items to its render phases (see [`phase`]).
    Queue,
    /// Sort the render phases, batch their items and upload per-instance data.
    Sort,
    /// Create bind groups for everything prepared above.
    PrepareBindGroups,
    /// Record the frame's render passes ([`driver::render_views`]).
    Render,
    /// Submit the frame and present it.
    Present,
    /// End-of-frame housekeeping.
    Cleanup,
}

/// The stages the render thread runs each frame, after extraction.
const RENDER_THREAD_STAGES: [RenderStage; 8] = [
    RenderStage::PrepareAssets,
    RenderStage::Prepare,
    RenderStage::Queue,
    RenderStage::Sort,
    RenderStage::PrepareBindGroups,
    RenderStage::Render,
    RenderStage::Present,
    RenderStage::Cleanup,
];

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
        let world = render_app.world_mut();
        world.push_manual_stage(RenderStage::Extract);
        for stage in RENDER_THREAD_STAGES {
            world.push_manual_stage(stage);
        }

        world.init_resource::<BindGroupLayouts>();
        world.init_resource::<RenderEntities>();
        world.add_system(despawn_orphaned_render_entities, RenderStage::Extract);
        world.init_resource::<ShaderLibrary>();

        world.add_system(resize_surface, RenderStage::Prepare);
        world.add_system(acquire_frame.after(resize_surface), RenderStage::Prepare);
        world.add_system(
            driver::render_views.run_if(frame_in_flight()),
            RenderStage::Render,
        );
        world.add_system(
            present_frame.run_if(frame_in_flight()),
            RenderStage::Present,
        );

        render_app.add_plugin(RenderPhasePlugin::<Background3d>::default())?;
        render_app.add_plugin(RenderPhasePlugin::<Opaque3d>::default())?;
        render_app.add_plugin(RenderPhasePlugin::<Overlay3d>::default())?;
        render_app.add_plugin(ViewPlugin)?;
        render_app.add_plugin(CameraPlugin)?;
        render_app.add_plugin(TransformPlugin)?;
        render_app.add_plugin(MeshPlugin)?;
        render_app.add_plugin(TexturePlugin)?;
        render_app.add_plugin(TonemapPlugin)?;

        render_app.add_event::<WindowResized>(RenderStage::Cleanup);
        render_app
            .world_mut()
            .add_system(forward_resized_events, RenderStage::Extract);

        render_app.set_extract(Box::new(extract::render_extract));

        main_app.add_sub_app::<RenderApp>(render_app);

        let mut extract_app = SubApp::new();
        extract_app.set_extract(Box::new(renderer_extract));
        main_app.add_sub_app::<RenderExtractApp>(extract_app);

        Ok(())
    }

    fn ready(&self, main_app: &App) -> bool {
        main_app.main_app().world().has_resource::<Window>()
            && main_app
                .main_app()
                .world()
                .has_resource::<EventQueue<WindowResized>>()
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

        let mut render_app = main_app.remove_sub_app::<RenderApp>().unwrap();

        surface::create_render_device(render_app.world_mut(), &window)?;
        render_app.world().insert_resource(window);

        render_app.finish_plugins();
        render_app.world_mut().initialize_systems();

        render_to_main_tx.send(render_app).unwrap();

        main_app.insert_resource(RenderAppChannels {
            main_to_render_tx,
            render_to_main_rx,
        });

        std::thread::spawn(move || {
            log::trace!("Entering render task main loop");

            loop {
                let Ok(mut render_app) = main_to_render_rx.recv() else {
                    break;
                };
                log::trace!("Received render app on render task");

                render_app.finish_plugins();

                let world = render_app.world_mut();
                for stage in RENDER_THREAD_STAGES {
                    log::trace!("Running render app stage: {stage:?}");
                    world.run_stage(stage).expect("render stage failed");
                }
                world.clear_trackers();

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

fn forward_resized_events(
    mut events: Extract<EventRx<WindowResized>>,
    mut render_events: EventTx<WindowResized>,
) {
    for event in events.by_ref() {
        render_events.send(event);
    }
}
