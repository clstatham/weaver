use weaver_app::{plugin::Plugin, App};
use weaver_core::color::Color;
use weaver_ecs::{
    prelude::Resource,
    storage::Ref,
    world::{World, WorldLock},
};
use weaver_util::prelude::Result;

use crate::{
    camera::ViewTarget,
    extract::{RenderResource, RenderResourcePlugin},
    graph::{RenderCtx, RenderGraphCtx, ViewNode},
    hdr::HdrRenderTarget,
    RenderApp, RenderLabel,
};

#[derive(Resource, Clone, Copy)]
pub struct ClearColor(pub Color);

impl RenderResource for ClearColor {
    type UpdateQuery = ();

    fn extract_render_resource(main_world: &mut World, _render_world: &mut World) -> Option<Self>
    where
        Self: Sized,
    {
        main_world.get_resource::<Self>().as_deref().cloned()
    }

    fn update_render_resource(
        &mut self,
        main_world: &mut World,
        _render_world: &mut World,
    ) -> Result<()> {
        if let Some(clear_color) = main_world.get_resource::<ClearColor>() {
            self.0 = clear_color.0;
        }
        Ok(())
    }
}

pub struct ClearColorPlugin(pub Color);

impl Default for ClearColorPlugin {
    fn default() -> Self {
        Self(Color::BLACK)
    }
}

impl Plugin for ClearColorPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(ClearColor(self.0));
        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app
            .add_plugin(RenderResourcePlugin::<ClearColor>::default())
            .unwrap();
        Ok(())
    }
}

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

    fn prepare(&mut self, render_world: &WorldLock) -> Result<()> {
        if let Some(clear_color) = render_world.get_resource::<ClearColor>() {
            self.color = clear_color.0;
        }
        Ok(())
    }

    fn run(
        &self,
        render_world: &WorldLock,
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

        let hdr_target = render_world.get_resource::<HdrRenderTarget>().unwrap();

        let _pass = render_ctx
            .command_encoder()
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_color"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: hdr_target.color_target(),
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
