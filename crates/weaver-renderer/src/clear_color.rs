use weaver_core::color::Color;
use weaver_ecs::world::World;

use crate::{graph::Render, target::RenderTarget, Renderer};

pub struct ClearColor {
    pub color: Color,
}

impl ClearColor {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Default for ClearColor {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
        }
    }
}

impl Render for ClearColor {
    fn render(
        &self,
        _world: &World,
        renderer: &Renderer,
        target: &RenderTarget,
    ) -> anyhow::Result<()> {
        let device = renderer.device();

        let view = target.texture_view(renderer).unwrap();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("clear_color"),
        });

        let color = wgpu::Color {
            r: self.color.r as f64,
            g: self.color.g as f64,
            b: self.color.b as f64,
            a: self.color.a as f64,
        };

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_color"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        renderer.enqueue_command_buffer(encoder.finish());

        Ok(())
    }
}
