use std::{collections::HashSet, ops::Range};

use assets::material_mesh::{
    GltfMaterialModelLoader, LoadedModelWithMaterials, ObjMaterialModelLoader,
};
use light::{PointLight, PointLightPlugin};
use material::{
    GltfMaterialLoader, GpuMaterial, Material, MaterialPlugin, BLACK_TEXTURE, ERROR_TEXTURE,
    WHITE_TEXTURE,
};
use render::{PbrLightingInformation, PbrMeshInstances, PbrNode, PbrNodeLabel, PbrRenderCommand};
use skybox::{Skybox, SkyboxNodeLabel, SkyboxNodePlugin, SkyboxPlugin};
use weaver_app::prelude::*;
use weaver_asset::{AddAsset, Assets, Handle};
use weaver_core::{texture::Texture, transform::Transform};
use weaver_ecs::{
    commands::WorldMut,
    component::{Res, ResMut},
    entity::Entity,
    query::{Query, With},
    storage::Ref,
};
use weaver_renderer::{
    bind_group::{BindGroup, ResourceBindGroupPlugin},
    camera::{insert_view_target, GpuCamera, ViewTarget},
    clear_color::{ClearColorLabel, ClearColorNode},
    draw_fn::{BinnedDrawItem, DrawFnId, DrawItem, FromDrawItemQuery},
    graph::{RenderGraphApp, ViewNodeRunner},
    hdr::HdrNodeLabel,
    mesh::GpuMesh,
    pipeline::RenderPipelinePlugin,
    render_command::AddRenderCommand,
    render_phase::{
        batch_and_prepare, BatchedInstanceBufferPlugin, BinnedRenderPhasePlugin, BinnedRenderPhases,
    },
    InitRenderResources, PreRender, RenderApp, WgpuDevice, WgpuQueue,
};
use weaver_util::prelude::*;

pub mod assets;
pub mod light;
pub mod material;
pub mod prepass;
pub mod render;
pub mod skybox;

pub mod prelude {
    pub use crate::assets::material_mesh::*;
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
        (mesh, material): (Ref<Handle<GpuMesh>>, Ref<Handle<BindGroup<GpuMaterial>>>),
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
    );
    type QueryFilter = With<Transform>;

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
        app.add_asset_loader::<Material, GltfMaterialLoader>();
        app.add_asset_loader::<LoadedModelWithMaterials, ObjMaterialModelLoader>();
        app.add_asset_loader::<LoadedModelWithMaterials, GltfMaterialModelLoader>();

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(MaterialPlugin)?;
        render_app.add_plugin(PointLightPlugin)?;
        render_app.add_plugin(SkyboxPlugin)?;
        render_app.add_plugin(SkyboxNodePlugin)?;

        render_app.add_plugin(ResourceBindGroupPlugin::<PbrLightingInformation>::default())?;

        render_app.add_plugin(RenderPipelinePlugin::<PbrNode>::default())?;

        render_app.add_plugin(BatchedInstanceBufferPlugin::<_, PbrRenderCommand>::default())?;
        render_app.insert_resource(PbrMeshInstances::default());

        render_app.add_render_command::<_, PbrRenderCommand>();

        render_app.add_plugin(BinnedRenderPhasePlugin::<PbrDrawItem>::default())?;

        render_app.add_system_after(extract_pbr_camera_phase, insert_view_target, PreRender);
        render_app.add_system_after(
            batch_and_prepare::<PbrDrawItem, PbrRenderCommand>,
            extract_pbr_camera_phase,
            PreRender,
        );

        render_app.add_render_main_graph_node::<ViewNodeRunner<ClearColorNode>>(ClearColorLabel);
        render_app.add_render_main_graph_node::<ViewNodeRunner<PbrNode>>(PbrNodeLabel);
        render_app.add_render_main_graph_edge(ClearColorLabel, PbrNodeLabel);
        render_app.add_render_main_graph_edge(PbrNodeLabel, SkyboxNodeLabel);
        render_app.add_render_main_graph_edge(SkyboxNodeLabel, HdrNodeLabel);

        render_app.add_system(init_pbr_lighting_information, InitRenderResources);
        render_app.add_system_after(
            update_pbr_lighting_information,
            init_pbr_lighting_information,
            InitRenderResources,
        );

        Ok(())
    }

    fn finish(&self, app: &mut App) -> Result<()> {
        let white_texture = Texture::from_rgba8(&[255, 255, 255, 255], 1, 1);
        let black_texture = Texture::from_rgba8(&[0, 0, 0, 255], 1, 1);
        let error_texture = Texture::from_rgba8(
            &[
                255, 0, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 255, 255,
            ],
            2,
            2,
        );
        let mut textures = app
            .main_app_mut()
            .world_mut()
            .get_resource_mut::<Assets<Texture>>()
            .unwrap();
        textures.insert_manual(white_texture, WHITE_TEXTURE.id());
        textures.insert_manual(black_texture, BLACK_TEXTURE.id());
        textures.insert_manual(error_texture, ERROR_TEXTURE.id());
        Ok(())
    }
}

pub(crate) fn init_pbr_lighting_information(
    mut world: WorldMut,
    _skybox: Res<Skybox>,
) -> Result<()> {
    if !world.has_resource::<PbrLightingInformation>() {
        world.init_resource::<PbrLightingInformation>();
    }
    Ok(())
}

pub(crate) fn update_pbr_lighting_information(
    mut lighting: ResMut<PbrLightingInformation>,
    lights: Query<(&PointLight, Option<&Transform>)>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    lighting.point_lights.buffer.clear();
    for (_, (light, transform)) in lights.iter() {
        if let Some(transform) = transform {
            let uniform = (*light, *transform).into();
            lighting.point_lights.buffer.push(uniform);
        } else {
            let uniform = (*light).into();
            lighting.point_lights.buffer.push(uniform);
        }
    }
    lighting.point_lights.buffer.enqueue_update(&device, &queue);
    Ok(())
}

pub fn extract_pbr_camera_phase(
    mut binned_phases: ResMut<BinnedRenderPhases<PbrDrawItem>>,
    cameras: Query<&GpuCamera, With<ViewTarget>>,
) -> Result<()> {
    let mut live_entities = HashSet::new();

    for (entity, camera) in cameras.iter() {
        if !camera.camera.active {
            continue;
        }

        log::trace!("Extracting PBR camera phase for entity: {:?}", entity);

        binned_phases.insert_or_clear(entity);
        live_entities.insert(entity);
    }

    binned_phases.retain(|entity, _| live_entities.contains(entity));

    Ok(())
}
