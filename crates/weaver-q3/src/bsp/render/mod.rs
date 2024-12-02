use extract::{extract_bsps, ExtractedBsp};
use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{component::Res, prelude::ResMut, query::Query};
use weaver_pbr::render::PbrLightingInformation;
use weaver_renderer::{
    bind_group::BindGroup,
    camera::{CameraBindGroup, ViewTarget},
    clear_color::render_clear_color,
    hdr::render_hdr,
    prelude::wgpu,
    resources::ActiveCommandEncoder,
    RenderStage,
};
use weaver_util::prelude::*;

use crate::shader::render::ShaderPipelineCache;

pub mod extract;

#[derive(Default)]
pub struct BspRenderable;

pub async fn render_bsps(
    bsp: Res<ExtractedBsp>,
    lighting_bind_group: Res<BindGroup<PbrLightingInformation>>,
    shader_pipeline_cache: Res<ShaderPipelineCache>,
    mut view_target: Query<(&'static ViewTarget, &'static BindGroup<CameraBindGroup>)>,
    mut encoder: ResMut<ActiveCommandEncoder>,
) {
    for (view_target, camera_bind_group) in view_target.iter() {
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
    }
}

pub struct BspRenderPlugin;

impl Plugin for BspRenderPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_system(extract_bsps, RenderStage::Extract);

        render_app.add_system(render_bsps, RenderStage::Render);

        render_app.main_app_mut().world_mut().order_systems(
            render_clear_color,
            render_bsps,
            RenderStage::Render,
        );

        render_app.main_app_mut().world_mut().order_systems(
            render_bsps,
            render_hdr,
            RenderStage::Render,
        );

        Ok(())
    }
}
