use extract::{extract_bsps, ExtractedBsp};
use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{
    component::Res,
    prelude::{SystemParamItem, World},
    query::QueryFetchItem,
};
use weaver_pbr::render::PbrLightingInformation;
use weaver_renderer::{
    bind_group::BindGroup,
    camera::{CameraBindGroup, ViewTarget},
    clear_color::ClearColorLabel,
    graph::{RenderGraphApp, RenderGraphCtx, ViewNode, ViewNodeRunner},
    hdr::HdrNodeLabel,
    prelude::wgpu,
    ExtractStage, RenderLabel,
};
use weaver_util::Result;

use crate::shader::render::ShaderPipelineCache;

pub mod extract;

#[derive(Debug, Clone, Copy)]
pub struct BspRenderNodeLabel;
impl RenderLabel for BspRenderNodeLabel {}

#[derive(Default)]
pub struct BspRenderNode;

impl ViewNode for BspRenderNode {
    type Param = (
        Res<'static, ExtractedBsp>,
        Res<'static, BindGroup<PbrLightingInformation>>,
        Res<'static, ShaderPipelineCache>,
    );

    type ViewQueryFetch = (&'static ViewTarget, &'static BindGroup<CameraBindGroup>);

    type ViewQueryFilter = ();

    fn run(
        &self,
        _render_world: &World,
        graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut weaver_renderer::graph::RenderCtx,
        (bsp, lighting_bind_group, shader_pipeline_cache): &SystemParamItem<Self::Param>,
        (view_target, camera_bind_group): &QueryFetchItem<Self::ViewQueryFetch>,
    ) -> Result<()> {
        let encoder = render_ctx.command_encoder();

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BSP Render Pass"),
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

        render_pass.set_bind_group(1, camera_bind_group.bind_group(), &[]);
        render_pass.set_bind_group(2, lighting_bind_group.bind_group(), &[]);

        render_pass.set_vertex_buffer(0, bsp.vbo.slice(..));

        bsp.key_paths.walk(&mut |stages| {
            let bind_group = stages.bind_group.as_ref().unwrap();
            let index_buffer = stages.index_buffer.as_ref().unwrap();
            let pipeline = shader_pipeline_cache.cache.get(&stages.key).unwrap();

            render_pass.set_pipeline(&pipeline.pipeline);

            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            render_pass.draw_indexed(0..stages.num_indices, 0, 0..1);
        });

        Ok(())
    }
}

pub struct BspRenderPlugin;

impl Plugin for BspRenderPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_system(extract_bsps, ExtractStage);

        render_app.add_render_main_graph_node::<ViewNodeRunner<BspRenderNode>>(BspRenderNodeLabel);
        render_app.add_render_main_graph_edge(ClearColorLabel, BspRenderNodeLabel);
        render_app.add_render_main_graph_edge(BspRenderNodeLabel, HdrNodeLabel);

        Ok(())
    }
}
