use std::ops::Range;

use weaver_app::plugin::Plugin;
use weaver_asset::Handle;
use weaver_core::{prelude::Mat4, transform::Transform};
use weaver_ecs::{
    component::Res,
    entity::EntityMap,
    prelude::{
        Component, Entity, QueryFetchItem, Reflect, Resource, SystemParamItem, World, WorldView,
    },
    storage::Ref,
};
use weaver_renderer::{
    bind_group::BindGroup,
    draw_fn::{BinnedDrawItem, DrawFnId, DrawFunctions, DrawItem, FromDrawItemQuery},
    graph::{RenderCtx, RenderGraphCtx, ViewNode},
    mesh::GpuMesh,
    prelude::wgpu,
    render_phase::{BinnedRenderPhases, GetBatchData},
    texture::GpuTexture,
    RenderLabel,
};
use weaver_util::Result;

use crate::prelude::GpuMaterial;

#[derive(Debug, Clone, Copy, Reflect, Component)]
pub struct DepthPrepass;

#[derive(Debug, Clone, Copy, Reflect, Component)]
pub struct NormalPrepass;

#[derive(Clone, Component, Default)]
pub struct PrepassTextures {
    pub depth: Option<GpuTexture>,
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
        (mesh, material): (Ref<Handle<GpuMesh>>, Ref<Handle<BindGroup<GpuMaterial>>>),
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
    pub instances: EntityMap<PrepassMeshInstance>,
}

impl GetBatchData for PrepassMeshInstances {
    type BufferData = Mat4;
    type UpdateQueryFetch = (
        &'static Handle<GpuMesh>,
        &'static Handle<BindGroup<GpuMaterial>>,
        &'static Transform,
    );
    type UpdateQueryFilter = ();

    fn update(&mut self, query: WorldView<Self::UpdateQueryFetch>) {
        self.instances.clear();
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

    fn get_batch_data(&self, query_item: Entity) -> Option<Self::BufferData> {
        self.instances
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
        Res<'static, BinnedRenderPhases<PrepassDrawItem>>,
        Res<'static, DrawFunctions<PrepassDrawItem>>,
    );

    type ViewQueryFetch = (
        &'static PrepassTextures,
        Option<&'static DepthPrepass>,
        Option<&'static NormalPrepass>,
    );
    type ViewQueryFilter = ();

    fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        let draw_fns = {
            render_world
                .remove_resource::<DrawFunctions<PrepassDrawItem>>()
                .unwrap()
        };
        let mut draw_fns_lock = draw_fns.write();
        draw_fns_lock.prepare(render_world)?;
        drop(draw_fns_lock);
        render_world.insert_resource(draw_fns);
        Ok(())
    }

    fn run(
        &self,
        render_world: &World,
        graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
        (binned_render_phases, draw_fns): &SystemParamItem<Self::Param>,
        (prepass_textures, _, _): &QueryFetchItem<Self::ViewQueryFetch>,
    ) -> Result<()> {
        let Some(phase) = binned_render_phases.get(&graph_ctx.view_entity) else {
            return Ok(());
        };

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

        let color_attachments = vec![normal_attachment];

        if !phase.is_empty() {
            let encoder = render_ctx.command_encoder();

            let mut draw_fns = draw_fns.write();

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Prepass Render Pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            phase.render(
                render_world,
                &mut render_pass,
                graph_ctx.view_entity,
                &mut draw_fns,
            )?;
        }

        Ok(())
    }
}

pub struct PrepassPlugin;

impl Plugin for PrepassPlugin {
    fn build(&self, _render_app: &mut weaver_app::App) -> Result<()> {
        Ok(())
    }
}
