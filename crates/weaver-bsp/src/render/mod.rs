use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

use encase::ShaderType;
use extract::{extract_bsps, ExtractedBsp, ExtractedBspNode};
use weaver_app::{plugin::Plugin, App};
use weaver_asset::{AddAsset, Assets, Handle};
use weaver_core::prelude::*;
use weaver_ecs::{
    component::{Res, ResMut},
    entity::Entity,
    prelude::{Query, QueryFetch, Resource, SystemParamItem, World},
    query::{QueryFetchItem, With},
    storage::Ref,
};
use weaver_pbr::{
    extract_pbr_camera_phase,
    material::GpuMaterial,
    prelude::SkyboxNodeLabel,
    render::{PbrLightingInformation, PbrNode},
};
use weaver_renderer::{
    bind_group::{BindGroup, BindGroupLayoutCache},
    camera::{CameraBindGroup, GpuCamera, ViewTarget},
    clear_color::ClearColorLabel,
    draw_fn::{BinnedDrawItem, DrawFnId, DrawFunctions, DrawItem, FromDrawItemQuery},
    graph::{RenderGraphApp, RenderGraphCtx, ViewNode, ViewNodeRunner},
    mesh::GpuMesh,
    pipeline::{CreateRenderPipeline, RenderPipelineCache, RenderPipelinePlugin},
    prelude::{wgpu, RenderPipeline, RenderPipelineLayout},
    render_command::{AddRenderCommand, RenderCommand},
    render_phase::{
        batch_and_prepare, BatchedInstanceBuffer, BatchedInstanceBufferPlugin,
        BinnedRenderPhasePlugin, BinnedRenderPhases, GetBatchData,
    },
    ExtractStage, PreRender, RenderLabel,
};
use weaver_util::prelude::Result;

pub mod extract;

pub struct BspDrawItem {
    pub key: BspDrawItemKey,
    pub entity: Entity,
    pub batch_range: Range<u32>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct BspDrawItemKey {
    pub bsp: Handle<ExtractedBsp>,
    pub draw_fn: DrawFnId,
}

impl FromDrawItemQuery<BspDrawItem> for BspDrawItemKey {
    fn from_draw_item_query(bsp: Ref<Handle<ExtractedBsp>>, draw_fn_id: DrawFnId) -> Self {
        Self {
            bsp: *bsp,
            draw_fn: draw_fn_id,
        }
    }
}

impl DrawItem for BspDrawItem {
    type QueryFetch = &'static Handle<ExtractedBsp>;

    type QueryFilter = ();

    fn entity(&self) -> Entity {
        self.entity
    }

    fn draw_fn(&self) -> DrawFnId {
        self.key.draw_fn
    }
}

impl BinnedDrawItem for BspDrawItem {
    type Key = BspDrawItemKey;

    type Instances = BspDrawItemInstances;

    fn new(key: Self::Key, entity: Entity, batch_range: Range<u32>) -> Self {
        Self {
            key,
            entity,
            batch_range,
        }
    }
}

#[derive(Clone, Copy, ShaderType)]
#[repr(C)]
pub struct BspDrawItemInstance {
    pub transform: Mat4,
}

#[derive(Default, Resource)]
pub struct BspDrawItemInstances {
    pub instances: HashMap<Entity, BspDrawItemInstance>,
}

impl GetBatchData for BspDrawItemInstances {
    type BufferData = BspDrawItemInstance;

    type UpdateQueryFetch = ();
    type UpdateQueryFilter = With<Handle<ExtractedBsp>>;

    fn update(&mut self, query: Query<Self::UpdateQueryFetch, Self::UpdateQueryFilter>) {
        self.instances.clear();
        for (entity, _) in query.iter() {
            let transform = Transform::IDENTITY.into();
            self.instances
                .insert(entity, BspDrawItemInstance { transform });
        }
    }

    fn get_batch_data(&self, query_item: Entity) -> Option<Self::BufferData> {
        self.instances.get(&query_item).copied()
    }
}

pub struct BspRenderCommand;

impl RenderCommand<BspDrawItem> for BspRenderCommand {
    type Param = (
        Res<'static, Assets<ExtractedBsp>>,
        Res<'static, Assets<GpuMesh>>,
        Res<'static, Assets<BindGroup<GpuMaterial>>>,
        Res<'static, RenderPipelineCache>,
        Res<'static, BindGroup<BatchedInstanceBuffer<BspDrawItem, BspRenderCommand>>>,
        Res<'static, BindGroup<PbrLightingInformation>>,
    );

    type ViewQueryFetch = (&'static BindGroup<CameraBindGroup>, &'static GpuCamera);

    type ViewQueryFilter = ();

    type ItemQueryFetch = ();

    type ItemQueryFilter = ();
    fn render<'w>(
        item: BspDrawItem,
        (camera_bind_group, camera): <Self::ViewQueryFetch as QueryFetch>::Item<'w>,
        _: Option<<Self::ItemQueryFetch as QueryFetch>::Item<'w>>,
        param: SystemParamItem<'w, '_, Self::Param>,
        render_pass: &mut wgpu::RenderPass<'w>,
    ) -> Result<()> {
        let (
            bsp_assets,
            mesh_assets,
            material_assets,
            pipeline_cache,
            instance_bind_group,
            lighting_bind_group,
        ) = param;
        let bsp_assets = bsp_assets.into_inner();
        let mesh_assets = mesh_assets.into_inner();
        let material_assets = material_assets.into_inner();
        let pipeline_cache = pipeline_cache.into_inner();
        let instance_bind_group = instance_bind_group.into_inner();
        let lighting_bind_group = lighting_bind_group.into_inner();
        let camera_bind_group = camera_bind_group.into_inner();
        let camera = camera.into_inner();

        let bsp = bsp_assets.get(item.key.bsp).unwrap();

        let pipeline = pipeline_cache.get_pipeline_for::<BspRenderNode>().unwrap();

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(1, camera_bind_group.bind_group(), &[]);
        render_pass.set_bind_group(2, instance_bind_group.bind_group(), &[]);
        render_pass.set_bind_group(3, lighting_bind_group.bind_group(), &[]);

        let inv_view = camera.camera.view_matrix.inverse();
        let camera_pos = inv_view.col(3).truncate();

        // figure out what leaf cluster the camera is in
        let mut camera_cluster = -1i32;
        let mut stack = vec![0];

        while let Some(node_index) = stack.pop() {
            let node = bsp.nodes[node_index].as_ref().unwrap();
            match node {
                ExtractedBspNode::Leaf { cluster, .. } => {
                    camera_cluster = *cluster as i32;
                    break;
                }
                ExtractedBspNode::Node {
                    back, front, plane, ..
                } => {
                    let dist = plane.normal.dot(camera_pos) - plane.distance;

                    if dist > 0.0 {
                        stack.push(*front);
                        stack.push(*back);
                    } else {
                        stack.push(*back);
                        stack.push(*front);
                    }
                }
            }
        }

        bsp.walk(0, &mut |_index, node| {
            match node {
                ExtractedBspNode::Leaf {
                    meshes_and_materials,
                    parent: _,
                    cluster,
                } => {
                    // check if the leaf's cluster is visible from the camera's cluster
                    let visible = {
                        if camera_cluster < 0 || (*cluster as i32) < 0 {
                            true
                        } else {
                            let vis = bsp.vis_data.vecs[camera_cluster as usize
                                * bsp.vis_data.size_vecs as usize
                                + *cluster / 8];
                            (vis & (1 << (*cluster % 8))) != 0
                        }
                    };
                    if visible {
                        for (mesh, material) in meshes_and_materials {
                            let mesh = mesh_assets.get(*mesh).unwrap();
                            let material = material_assets.get(*material).unwrap();
                            let mesh = mesh.into_inner();
                            let material = material.into_inner();

                            render_pass.set_bind_group(0, material.bind_group(), &[]);
                            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                            render_pass.set_index_buffer(
                                mesh.index_buffer.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );

                            render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                        }
                    }
                }
                ExtractedBspNode::Node { .. } => {}
            }
        });

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BspRenderNodeLabel;
impl RenderLabel for BspRenderNodeLabel {}

#[derive(Default)]
pub struct BspRenderNode;

impl ViewNode for BspRenderNode {
    type Param = (
        Res<'static, BinnedRenderPhases<BspDrawItem>>,
        Res<'static, DrawFunctions<BspDrawItem>>,
    );

    type ViewQueryFetch = &'static ViewTarget;

    type ViewQueryFilter = ();

    fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        let draw_fns = render_world
            .remove_resource::<DrawFunctions<BspDrawItem>>()
            .unwrap();
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
        render_ctx: &mut weaver_renderer::graph::RenderCtx,
        (binned_phases, draw_functions): &SystemParamItem<Self::Param>,
        view_target: &QueryFetchItem<Self::ViewQueryFetch>,
    ) -> Result<()> {
        let Some(phase) = binned_phases.get(&graph_ctx.view_entity) else {
            log::debug!("No binned render phase found for BspRenderNode");
            return Ok(());
        };

        let mut draw_functions_lock = draw_functions.write();

        if !phase.is_empty() {
            let encoder = render_ctx.command_encoder();

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PBR Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_target.color_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view_target.depth_target,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            log::trace!(
                "Rendering BSP phase for view entity {:?}",
                graph_ctx.view_entity
            );

            phase.render(
                render_world,
                &mut render_pass,
                graph_ctx.view_entity,
                &mut draw_functions_lock,
            )?;
        } else {
            log::trace!(
                "BSP render phase is empty for view entity {:?}",
                graph_ctx.view_entity
            );
        }

        Ok(())
    }
}

impl CreateRenderPipeline for BspRenderNode {
    fn create_render_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> RenderPipelineLayout
    where
        Self: Sized,
    {
        let material_layout = bind_group_layout_cache.get_or_create::<GpuMaterial>(device);
        let instance_layout = bind_group_layout_cache
            .get_or_create::<BatchedInstanceBuffer<BspDrawItem, BspRenderCommand>>(device);
        let lighting_layout =
            bind_group_layout_cache.get_or_create::<PbrLightingInformation>(device);
        let camera_layout = bind_group_layout_cache.get_or_create::<CameraBindGroup>(device);

        RenderPipelineLayout::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("BspRenderNode Pipeline Layout"),
                bind_group_layouts: &[
                    &material_layout,
                    &camera_layout,
                    &instance_layout,
                    &lighting_layout,
                ],
                push_constant_ranges: &[],
            }),
        )
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        cached_layout: &wgpu::PipelineLayout,
    ) -> RenderPipeline
    where
        Self: Sized,
    {
        // we're lazy here
        PbrNode::create_render_pipeline(device, cached_layout)
    }
}

pub struct BspRenderPlugin;

impl Plugin for BspRenderPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_asset::<ExtractedBsp>();
        render_app.add_system(extract_bsps, ExtractStage);

        render_app.add_plugin(RenderPipelinePlugin::<BspRenderNode>::default())?;

        render_app
            .add_plugin(BatchedInstanceBufferPlugin::<BspDrawItem, BspRenderCommand>::default())?;
        render_app.insert_resource(BspDrawItemInstances::default());

        render_app.add_render_command::<_, BspRenderCommand>();

        render_app.add_plugin(BinnedRenderPhasePlugin::<BspDrawItem>::default())?;

        render_app.add_system_after(
            extract_bsp_camera_phase,
            extract_pbr_camera_phase,
            PreRender,
        );
        render_app.add_system_after(
            batch_and_prepare::<BspDrawItem, BspRenderCommand>,
            extract_bsp_camera_phase,
            PreRender,
        );

        render_app.add_render_main_graph_node::<ViewNodeRunner<BspRenderNode>>(BspRenderNodeLabel);
        render_app.add_render_main_graph_edge(ClearColorLabel, BspRenderNodeLabel);
        render_app.add_render_main_graph_edge(BspRenderNodeLabel, SkyboxNodeLabel);

        Ok(())
    }

    fn finish(&self, app: &mut App) -> Result<()> {
        app.init_resource::<BatchedInstanceBuffer<BspDrawItem, BspRenderCommand>>();
        Ok(())
    }
}

pub fn extract_bsp_camera_phase(
    mut binned_phases: ResMut<BinnedRenderPhases<BspDrawItem>>,
    cameras: Query<&GpuCamera, With<ViewTarget>>,
) -> Result<()> {
    let mut live_cameras = HashSet::new();
    for (entity, camera) in cameras.iter() {
        if !camera.camera.active {
            continue;
        }

        binned_phases.insert_or_clear(entity);
        live_cameras.insert(entity);
    }

    binned_phases.retain(|entity, _| live_cameras.contains(entity));

    Ok(())
}
