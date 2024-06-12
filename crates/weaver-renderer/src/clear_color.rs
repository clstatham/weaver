use std::sync::Arc;

use weaver_core::color::Color;
use weaver_ecs::{prelude::Component, world::World};

use crate::{
    graph::{Render, Slot},
    Renderer,
};

#[derive(Component)]
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
        _world: Arc<World>,
        renderer: &Renderer,
        input_slots: &[Slot],
    ) -> anyhow::Result<Vec<Slot>> {
        log::trace!("ClearColor::render");
        let device = renderer.device();

        let Slot::Texture(color_target) = &input_slots[0] else {
            return Err(anyhow::anyhow!("Expected a texture slot"));
        };

        let Slot::Texture(depth_target) = &input_slots[1] else {
            return Err(anyhow::anyhow!("Expected a texture slot"));
        };

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
                    view: color_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_target,
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

        renderer.enqueue_command_buffer(encoder.finish());

        Ok(vec![
            Slot::Texture(color_target.clone()),
            Slot::Texture(depth_target.clone()),
        ])
    }
}
