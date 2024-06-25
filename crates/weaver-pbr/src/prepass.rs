use std::{collections::HashMap, ops::Range};

use weaver_app::plugin::Plugin;
use weaver_asset::Handle;
use weaver_core::{prelude::Mat4, transform::Transform};
use weaver_ecs::{
    component::Res,
    prelude::{
        Component, Entity, QueryFetchItem, Reflect, Resource, SystemParamItem, World, WorldLock,
    },
    storage::Ref,
};
use weaver_renderer::{
    bind_group::BindGroup,
    camera::ViewTarget,
    draw_fn::{BinnedDrawItem, DrawFnId, DrawFunctions, DrawItem, FromDrawItemQuery},
    graph::{RenderCtx, RenderGraphCtx, ViewNode},
    mesh::GpuMesh,
    prelude::wgpu,
    render_phase::{BinnedRenderPhases, GetBatchData},
    texture::GpuTexture,
    RenderLabel,
};
use weaver_util::prelude::Result;

use crate::prelude::GpuMaterial;

#[derive(Debug, Clone, Copy, Reflect, Component)]
pub struct DepthPrepass;

#[derive(Debug, Clone, Copy, Reflect, Component)]
pub struct NormalPrepass;

#[derive(Clone, Reflect, Component, Default)]
pub struct PrepassTextures {
    #[reflect(ignore)]
    pub depth: Option<GpuTexture>,
    #[reflect(ignore)]
    pub normal: Option<GpuTexture>,
}

impl PrepassTextures {
    pub fn depth_view(&self) -> Option<&wgpu::TextureView> {
        self.depth.as_ref().map(|t| &*t.view)
    }

    pub fn normal_view(&self) -> Option<&wgpu::TextureView> {
        self.normal.as_ref().map(|t| &*t.view)
    }
}

pub struct PrepassDrawItem {
    pub key: PrepassKey,
    pub entity: Entity,
    pub batch_range: Range<u32>,
}

#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord, Debug)]
pub struct PrepassKey {
    pub draw_fn: DrawFnId,
    pub mesh: Handle<GpuMesh>,
    pub material: Handle<BindGroup<GpuMaterial>>,
}

impl FromDrawItemQuery<PrepassDrawItem> for PrepassKey {
    fn from_draw_item_query(
        (mesh, material): (
            weaver_ecs::storage::Ref<Handle<GpuMesh>>,
            weaver_ecs::storage::Ref<Handle<BindGroup<GpuMaterial>>>,
        ),
        draw_fn_id: DrawFnId,
    ) -> Self {
        Self {
            draw_fn: draw_fn_id,
            mesh: *mesh,
            material: *material,
        }
    }
}

impl DrawItem for PrepassDrawItem {
    type QueryFetch = (
        &'static Handle<GpuMesh>,
        &'static Handle<BindGroup<GpuMaterial>>,
    );
    type QueryFilter = ();

    fn entity(&self) -> Entity {
        self.entity
    }

    fn draw_fn(&self) -> DrawFnId {
        self.key.draw_fn
    }
}

pub struct PrepassMeshInstance {
    pub mesh: Handle<GpuMesh>,
    pub material: Handle<BindGroup<GpuMaterial>>,
    pub transform: Transform,
}

#[derive(Default, Resource)]
pub struct PrepassMeshInstances {
    pub instances: HashMap<Entity, PrepassMeshInstance>,
}

impl GetBatchData for PrepassMeshInstances {
    type Param = Res<PrepassMeshInstances>;
    type BufferData = Mat4;
    type UpdateQuery = (
        &'static Handle<GpuMesh>,
        &'static Handle<BindGroup<GpuMaterial>>,
        &'static Transform,
    );

    fn update_from_world(&mut self, render_world: &World) {
        self.instances.clear();
        let query = render_world.query::<Self::UpdateQuery>();
        for (entity, (mesh, material, transform)) in query.iter() {
            self.instances.insert(
                entity,
                PrepassMeshInstance {
                    mesh: *mesh,
                    material: *material,
                    transform: *transform,
                },
            );
        }
    }

    fn get_batch_data(
        param: &SystemParamItem<Self::Param>,
        query_item: Entity,
    ) -> Option<Self::BufferData> {
        param
            .instances
            .get(&query_item)
            .map(|instance| instance.transform.matrix())
    }
}

impl BinnedDrawItem for PrepassDrawItem {
    type Key = PrepassKey;
    type Instances = PrepassMeshInstances;

    fn new(key: Self::Key, entity: Entity, batch_range: Range<u32>) -> Self {
        Self {
            key,
            entity,
            batch_range,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrepassNodeLabel;
impl RenderLabel for PrepassNodeLabel {}

#[derive(Debug, Clone, Copy, Default)]
pub struct PrepassNode;

impl ViewNode for PrepassNode {
    type Param = (
        Res<BinnedRenderPhases<PrepassDrawItem>>,
        Res<DrawFunctions<PrepassDrawItem>>,
        Res<PrepassMeshInstances>,
    );

    type ViewQueryFetch = (
        &'static ViewTarget,
        &'static PrepassTextures,
        Option<&'static DepthPrepass>,
        Option<&'static NormalPrepass>,
    );
    type ViewQueryFilter = ();

    fn run(
        &self,
        render_world: &WorldLock,
        graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
        (binned_render_phases, draw_fns, prepass_mesh_instances): &SystemParamItem<Self::Param>,
        (view_target, prepass_textures, depth_prepass, normal_prepass): &QueryFetchItem<
            Self::ViewQueryFetch,
        >,
    ) -> Result<()> {
        let Some(phase) = binned_render_phases.get(&graph_ctx.view_entity) else {
            return Ok(());
        };

        draw_fns.write().prepare(render_world).unwrap();

        let depth_stencil_attachment =
            prepass_textures
                .depth_view()
                .map(|view| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                });

        let normal_attachment =
            prepass_textures
                .normal_view()
                .map(|view| wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                });

        let color_attachments = vec![normal_attachment.map(|a| a.into())];

        if !phase.is_empty() {
            let encoder = render_ctx.command_encoder();

            let mut draw_fns = draw_fns.write_arc();

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Prepass Render Pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            phase.render(
                &render_world,
                &mut render_pass,
                graph_ctx.view_entity,
                &mut draw_fns,
            )?;

            drop(render_pass);
            drop(draw_fns);
        }

        Ok(())
    }
}

pub struct PrepassPlugin;

impl Plugin for PrepassPlugin {
    fn build(&self, render_app: &mut weaver_app::App) -> Result<()> {
        Ok(())
    }
}
