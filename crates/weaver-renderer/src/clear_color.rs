use weaver_app::{plugin::Plugin, App};
use weaver_core::color::Color;
use weaver_ecs::{component::Res, prelude::ResMut, query::Query};
use weaver_util::prelude::*;

use crate::{
    camera::ViewTarget,
    extract::{ExtractResource, ExtractResourcePlugin},
    hdr::HdrRenderTarget,
    resources::ActiveCommandEncoder,
    RenderApp, RenderStage,
};

#[derive(Clone, Copy)]
pub struct ClearColor(pub Color);

impl ExtractResource for ClearColor {
    type Source = ClearColor;
    fn extract_render_resource(source: &Self::Source) -> Self
    where
        Self: Sized,
    {
        *source
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
            .add_plugin(ExtractResourcePlugin::<ClearColor>::default())
            .unwrap();

        render_app
            .world_mut()
            .add_system(render_clear_color, RenderStage::Render);
        Ok(())
    }
}

pub struct ClearColorRenderable {
    pub color: Color,
}

impl ClearColorRenderable {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Default for ClearColorRenderable {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
        }
    }
}

pub async fn render_clear_color(
    hdr_target: Res<HdrRenderTarget>,
    mut view_query: Query<&ViewTarget>,
    mut command_encoder: ResMut<ActiveCommandEncoder>,
    clear_color: Res<ClearColor>,
) {
    let color = wgpu::Color {
        r: clear_color.0.r as f64,
        g: clear_color.0.g as f64,
        b: clear_color.0.b as f64,
        a: clear_color.0.a as f64,
    };

    for view_query in view_query.iter() {
        let _pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
    }
}
