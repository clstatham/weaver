//! The wgpu rendering core: a second world on its own thread, plus the machinery other crates
//! plug rendering into.
//!
//! This crate doesn't draw any scene content itself. Materials, lights and the skybox live in
//! `weaver-pbr`, debug lines in `weaver-gizmos`, UI in `weaver-egui`; they all hook into the
//! stages, assets, bind groups and phases defined here.
//!
//! # Two worlds, two threads
//!
//! [`RendererPlugin`] builds a second [`SubApp`] with its own [`World`], the render world, and
//! runs it on a dedicated render thread. The main world only knows about the window; the wgpu
//! instance, device, queue and surface ([`WgpuDevice`], [`WgpuQueue`], ...) exist only in the
//! render world, so no GPU work happens on the main thread.
//!
//! The render [`SubApp`] is passed back and forth between the threads over two bounded channels
//! ([`RenderAppChannels`]). Each frame the main thread's [`RenderExtractApp`] waits for the render
//! thread to hand the sub-app back, runs [`RenderStage::Extract`] on it (this is the only moment
//! the two worlds touch), and sends it off again. The render thread then runs the remaining
//! stages while the main world simulates its next frame. That gives one frame of overlap, and
//! also means the main thread stalls in extract if rendering takes longer than simulating.
//!
//! # Extraction
//!
//! During extract, the main world is moved into the render world as a [`MainWorld`] resource
//! (see [`extract::render_extract`]), and systems read it through [`extract::Extract`]
//! parameters, such as `Extract<Query<..>>`. Change detection inside `Extract` is relative to the
//! system's previous extract run, so `Changed<..>` means "changed since the last extraction".
//!
//! Components are copied with [`extract::ExtractComponent`] and resources with
//! [`extract::ExtractResource`]. Each main-world entity that gets extracted has a mirror entity in
//! the render world (a [`sync::MainEntity`] component, found through [`sync::RenderEntities`]),
//! which is despawned when the main entity is.
//!
//! # Stages
//!
//! Every frame runs the [`RenderStage`]s in this order: `Extract`, `PrepareAssets`, `Prepare`,
//! `Queue`, `Sort`, `PrepareBindGroups`, `Render`, `Present`, `Cleanup`. In terms of the usual
//! flow that is extract, prepare, queue, sort, render, present. They are manual stages, run by
//! the render thread and not by `World::update`.
//!
//! # Render assets
//!
//! A [`asset::RenderAsset`] is the GPU version of a main-world asset (a mesh, an image, a
//! material). Changed source assets are cloned during extract and turned into GPU assets in
//! `PrepareAssets`, stored in [`asset::RenderAssets`] under the source asset's `AssetId`.
//! Preparation can ask to be retried next frame when a dependency isn't ready.
//!
//! # Bind groups and buffers
//!
//! A type implements [`bind_group::AsBindGroup`] to describe its layout and bindings in one
//! place. [`bind_group::BindGroup<T>`] remembers which GPU buffers and textures it was built from
//! (they carry a [`GpuResourceId`]) and gets rebuilt when any of them is reallocated.
//! [`buffer::GpuBufferVec`] is the usual way to keep a CPU-side array in sync with a GPU buffer.
//!
//! # Views, phases and the frame
//!
//! Each active [`camera::Camera`] becomes an [`view::ExtractedView`] with its own HDR and depth
//! [`view::ViewTarget`] and a [`phase::RenderPhase`] per phase type. Plugins queue items into the
//! phases during `Queue`; `Sort` sorts them and batches opaque meshes. In `Render`,
//! [`driver::render_views`] draws each view's `Background3d`, `Opaque3d` and `Overlay3d` phases
//! in that order into the view's HDR target, then tonemaps the primary camera's view onto the
//! swapchain texture ([`tonemap`]). `Present` submits the frame's command buffers and presents.
//! Surface acquire and present live in [`surface`]; draw systems are gated on
//! [`surface::frame_in_flight`] so a frame where no swapchain texture was available is skipped.
//!
//! # Plugging in
//!
//! A rendering plugin is added to the render [`SubApp`] and typically registers, in order of
//! appearance: [`extract::ExtractComponentPlugin`]s and [`asset::RenderAssetPlugin`]s for its data,
//! a [`pipeline::InitRenderResourcePlugin`] for its pipelines (shaders come from
//! [`shader::ShaderLibrary`]), bind group plugins for its uniforms, a queue system that pushes
//! phase items, and a [`phase::DrawFunction`] registered with
//! [`phase::AddDrawFunction::add_draw_function`]. Anything that draws on top of the swapchain
//! texture after tonemapping (egui) runs in `Render` after [`driver::render_views`].
//!
//! [`World`]: weaver_ecs::world::World

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

define_atomic_id!(
    /// Identifies one GPU allocation (a buffer or texture).
    ///
    /// A new id is minted whenever a resource is (re)created, so a bind group can tell that a
    /// resource it references was replaced by comparing ids, instead of relying on someone
    /// remembering to mark it stale.
    GpuResourceId
);

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

/// The items most renderer users need, plus `encase` and `wgpu`.
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

/// Label of the render world's [`SubApp`]. Only valid while the app is being built: once the
/// renderer finishes, the sub-app moves to the render thread.
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

/// The wgpu instance. Render world only.
pub struct WgpuInstance {
    /// The wrapped instance.
    pub instance: wgpu::Instance,
}

impl Deref for WgpuInstance {
    type Target = wgpu::Instance;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

/// The GPU adapter the device was created from. Render world only.
pub struct WgpuAdapter {
    /// The wrapped adapter.
    pub adapter: wgpu::Adapter,
}

impl Deref for WgpuAdapter {
    type Target = wgpu::Adapter;

    fn deref(&self) -> &Self::Target {
        &self.adapter
    }
}

/// The wgpu device. Render world only, so main-world code can't create GPU resources.
pub struct WgpuDevice {
    /// The wrapped device.
    pub device: wgpu::Device,
}

impl Deref for WgpuDevice {
    type Target = wgpu::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

/// The wgpu queue. Render world only.
pub struct WgpuQueue {
    /// The wrapped queue.
    pub queue: wgpu::Queue,
}

impl Deref for WgpuQueue {
    type Target = wgpu::Queue;

    fn deref(&self) -> &Self::Target {
        &self.queue
    }
}

/// Label of the main-thread sub-app whose only job is to run [`RendererPlugin`]'s extraction: it
/// takes the render [`SubApp`] back from the render thread, extracts into it and sends it off
/// again.
pub struct RenderExtractApp;
impl AppLabel for RenderExtractApp {}

/// Main-world resource holding the channels the render [`SubApp`] travels over between the main
/// thread and the render thread.
pub struct RenderAppChannels {
    /// Sends the render sub-app to the render thread once extraction is done.
    pub main_to_render_tx: crossbeam_channel::Sender<SubApp>,
    /// Receives the render sub-app back after the render thread finished a frame.
    pub render_to_main_rx: crossbeam_channel::Receiver<SubApp>,
}

/// An empty world kept as a main-world resource so extraction has something to swap in while the
/// real main world is inside [`MainWorld`].
#[derive(Default)]
pub struct ScratchMainWorld(World);

/// The main world, as seen from the render world during [`RenderStage::Extract`]. Read it through
/// [`extract::Extract`] rather than directly.
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

/// Sets up the render world, the render thread and everything in [`crate`]'s overview.
///
/// Add it to the main app. It isn't ready until the main world has a [`Window`] and a
/// [`WindowResized`] event queue (both come from `weaver-winit`); `finish` then creates the wgpu
/// device and surface and starts the render thread. Add [`clear_color::ClearColorPlugin`] after
/// it.
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
                    // FIXME: a stage Err panics the render thread; the main thread then panics on
                    // recv().unwrap() in renderer_extract
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

// Extract fn of the `RenderExtractApp`: blocks until the render thread is done with the last frame.
fn renderer_extract(main_world: &mut World, _world: &mut World) -> Result<()> {
    let channels = main_world.remove_resource::<RenderAppChannels>().unwrap();
    // TODO: no graceful shutdown; this unwrap panics the main thread if the render thread died
    let mut render_app = channels.render_to_main_rx.recv().unwrap();
    log::trace!("Received render app on main thread");
    render_app.extract_from(main_world).unwrap();
    log::trace!("Sending render app back to render thread");
    channels.main_to_render_tx.send(render_app).unwrap();
    main_world.insert_resource(channels);

    Ok(())
}

// Copies the main world's resize events into the render world, where `resize_surface` reads them.
fn forward_resized_events(
    mut events: Extract<EventRx<WindowResized>>,
    mut render_events: EventTx<WindowResized>,
) {
    for event in events.by_ref() {
        render_events.send(event);
    }
}
