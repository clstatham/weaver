use std::{collections::HashSet, ops::Range};

use light::PointLightPlugin;
use material::{GpuMaterial, MaterialPlugin};
use render::{PbrLightingInformation, PbrMeshInstances, PbrNode, PbrNodeLabel, PbrRenderCommand};
use skybox::{SkyboxNodeLabel, SkyboxNodePlugin, SkyboxPlugin};
use weaver_app::prelude::*;
use weaver_asset::Handle;
use weaver_core::transform::Transform;
use weaver_ecs::{
    component::ResMut, entity::Entity, query::Query, storage::Ref, system::ExclusiveSystemMarker,
};
use weaver_renderer::{
    bind_group::{BindGroup, ResourceBindGroupPlugin},
    camera::GpuCamera,
    clear_color::{ClearColorLabel, ClearColorNode},
    draw_fn::{BinnedDrawItem, DrawFnId, DrawItem, FromDrawItemQuery},
    extract::{extract_render_component, RenderResourcePlugin},
    graph::{RenderGraphApp, ViewNodeRunner},
    hdr::HdrNodeLabel,
    mesh::GpuMesh,
    pipeline::RenderPipelinePlugin,
    render_command::AddRenderCommand,
    render_phase::{
        batch_and_prepare, BatchedInstanceBufferPlugin, BinnedRenderPhasePlugin, BinnedRenderPhases,
    },
    Extract, PreRender, RenderApp,
};
use weaver_util::prelude::*;

pub mod light;
pub mod material;
pub mod prepass;
pub mod render;
pub mod skybox;

pub mod prelude {
    pub use crate::light::*;
    pub use crate::material::*;
    pub use crate::skybox::*;
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
    type QueryFilter = ();

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

pub struct PbrPlugin;

impl Plugin for PbrPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(MaterialPlugin)?;
        render_app.add_plugin(PointLightPlugin)?;
        render_app.add_plugin(SkyboxPlugin)?;
        render_app.add_plugin(SkyboxNodePlugin)?;

        render_app.insert_resource(PbrLightingInformation);
        render_app.add_plugin(RenderResourcePlugin::<PbrLightingInformation>::default())?;
        render_app.add_plugin(ResourceBindGroupPlugin::<PbrLightingInformation>::default())?;

        render_app.add_plugin(RenderPipelinePlugin::<PbrNode>::default())?;

        render_app.add_plugin(BatchedInstanceBufferPlugin::<_, PbrRenderCommand>::default())?;
        render_app.insert_resource(PbrMeshInstances::default());

        render_app.add_render_command::<_, PbrRenderCommand>();

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

        render_app.add_render_main_graph_node::<ViewNodeRunner<ClearColorNode>>(ClearColorLabel);
        render_app.add_render_main_graph_node::<ViewNodeRunner<PbrNode>>(PbrNodeLabel);
        render_app.add_render_main_graph_edge(ClearColorLabel, PbrNodeLabel);
        render_app.add_render_main_graph_edge(PbrNodeLabel, SkyboxNodeLabel);
        render_app.add_render_main_graph_edge(SkyboxNodeLabel, HdrNodeLabel);

        Ok(())
    }
}

fn extract_pbr_camera_phase(
    mut binned_phases: ResMut<BinnedRenderPhases<PbrDrawItem>>,
    cameras: Query<&GpuCamera>,
    _marker: ExclusiveSystemMarker,
) -> Result<()> {
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
