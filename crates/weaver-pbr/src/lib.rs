use std::{collections::HashSet, ops::Range};

use light::PointLightPlugin;
use material::{GpuMaterial, MaterialPlugin};
use render::{PbrMeshInstances, PbrNode, PbrNodeLabel, PbrRenderCommand};
use weaver_app::prelude::*;
use weaver_asset::Handle;
use weaver_core::transform::Transform;
use weaver_ecs::{entity::Entity, storage::Ref, world::ReadWorld};
use weaver_renderer::{
    bind_group::BindGroup,
    camera::GpuCamera,
    clear_color::{ClearColorLabel, ClearColorNode},
    draw_fn::{BinnedDrawItem, DrawFnId, DrawFnsApp, DrawItem, FromDrawItemQuery},
    extract::extract_render_component,
    graph::{RenderGraphApp, ViewNodeRunner},
    hdr::HdrNodeLabel,
    mesh::GpuMesh,
    pipeline::RenderPipelinePlugin,
    render_command::RenderCommandState,
    render_phase::{
        batch_and_prepare, BatchedInstanceBufferPlugin, BinnedRenderPhasePlugin, BinnedRenderPhases,
    },
    Extract, PreRender, RenderApp, RenderLabel,
};
use weaver_util::prelude::*;

pub mod light;
pub mod material;
pub mod render;

pub mod prelude {
    pub use crate::light::*;
    pub use crate::material::*;
    pub use crate::PbrPlugin;
}

pub struct PbrDrawItem {
    pub key: PbrBinKey,
    pub entity: Entity,
    pub batch_range: Range<u32>,
}

#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord, Debug)]
pub struct PbrBinKey {
    pub mesh: Handle<GpuMesh>,
    pub material: Handle<BindGroup<GpuMaterial>>,
    pub draw_fn: DrawFnId,
}

impl FromDrawItemQuery<PbrDrawItem> for PbrBinKey {
    fn from_draw_item_query(
        (mesh, material, _): (
            Ref<Handle<GpuMesh>>,
            Ref<Handle<BindGroup<GpuMaterial>>>,
            Ref<Transform>,
        ),
        draw_fn_id: DrawFnId,
    ) -> Self {
        Self {
            mesh: *mesh,
            material: *material,
            draw_fn: draw_fn_id,
        }
    }
}

impl DrawItem for PbrDrawItem {
    type QueryFetch = (
        &'static Handle<GpuMesh>,
        &'static Handle<BindGroup<GpuMaterial>>,
        &'static Transform,
    );

    fn entity(&self) -> Entity {
        self.entity
    }

    fn draw_fn(&self) -> DrawFnId {
        self.key.draw_fn
    }
}

impl BinnedDrawItem for PbrDrawItem {
    type Key = PbrBinKey;
    type Instances = PbrMeshInstances;

    fn new(key: Self::Key, entity: Entity, batch_range: Range<u32>) -> Self {
        Self {
            key,
            entity,
            batch_range,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PbrSubGraph;
impl RenderLabel for PbrSubGraph {}

pub struct PbrPlugin;

impl Plugin for PbrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(MaterialPlugin)?;
        render_app.add_plugin(PointLightPlugin)?;
        render_app.add_plugin(RenderPipelinePlugin::<PbrNode>::default())?;

        render_app
            .add_plugin(BatchedInstanceBufferPlugin::<PbrDrawItem, PbrRenderCommand>::default())?;
        render_app.insert_resource(PbrMeshInstances::default());

        let pbr_draw_fn =
            RenderCommandState::<PbrDrawItem, PbrRenderCommand>::new(&render_app.read_world());
        render_app.add_draw_fn(pbr_draw_fn);

        render_app.add_plugin(BinnedRenderPhasePlugin::<PbrDrawItem>::default())?;

        render_app.add_system_after(
            extract_pbr_camera_phase,
            extract_render_component::<GpuCamera>,
            Extract,
        );
        render_app.add_system(
            batch_and_prepare::<PbrDrawItem, PbrRenderCommand>,
            PreRender,
        );

        render_app.add_render_sub_graph(PbrSubGraph);
        render_app.add_render_sub_graph_node::<ViewNodeRunner<ClearColorNode>>(
            PbrSubGraph,
            ClearColorLabel,
        );
        render_app.add_render_sub_graph_node::<ViewNodeRunner<PbrNode>>(PbrSubGraph, PbrNodeLabel);
        render_app.add_render_sub_graph_edge(PbrSubGraph, ClearColorLabel, PbrNodeLabel);
        render_app.add_render_main_graph_edge(PbrSubGraph, HdrNodeLabel);

        Ok(())
    }
}

fn extract_pbr_camera_phase(render_world: ReadWorld) -> Result<()> {
    let mut binned_phases = render_world
        .get_resource_mut::<BinnedRenderPhases<PbrDrawItem>>()
        .unwrap();

    let cameras = render_world.query::<&GpuCamera>();

    let mut live_entities = HashSet::new();

    for (entity, camera) in cameras.iter() {
        if !camera.camera.active {
            continue;
        }

        binned_phases.insert_or_clear(entity);
        live_entities.insert(entity);
    }

    binned_phases.retain(|entity, _| live_entities.contains(entity));

    Ok(())
}
