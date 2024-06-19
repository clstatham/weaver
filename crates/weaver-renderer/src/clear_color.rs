use weaver_core::color::Color;
use weaver_ecs::{storage::Ref, world::World};
use weaver_util::prelude::Result;

use crate::{
    camera::ViewTarget,
    graph::{RenderCtx, RenderGraphCtx, RenderLabel, ViewNode},
};

#[derive(Debug, Clone, Copy)]
pub struct ClearColorLabel;
impl RenderLabel for ClearColorLabel {}

pub struct ClearColorNode {
    pub color: Color,
}

impl ClearColorNode {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Default for ClearColorNode {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
        }
    }
}

impl ViewNode for ClearColorNode {
    type ViewQueryFetch = &'static ViewTarget;
    type ViewQueryFilter = ();

    fn run(
        &self,
        _render_world: &World,
        _graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
        view_query: &Ref<ViewTarget>,
    ) -> Result<()> {
        let color = self.color;
        let color = wgpu::Color {
            r: color.r as f64,
            g: color.g as f64,
            b: color.b as f64,
            a: color.a as f64,
        };

        let _pass = render_ctx.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear_color"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view_query.color_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view_query.depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        Ok(())
    }
}
