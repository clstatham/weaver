use weaver_asset::{Assets, Handle};
use weaver_ecs::{entity::Entity, prelude::World, query::Query};
use weaver_renderer::{
    bind_group::{BindGroup, CreateBindGroup},
    camera::GpuCamera,
    graph::{Render, Slot},
    mesh::GpuMesh,
    prelude::*,
    shader::Shader,
    transform::GpuTransform,
};
use weaver_util::prelude::{bail, Result};

use crate::{light::GpuPointLightArray, material::GpuMaterial};

#[derive(Default)]
pub struct PbrNode {
    pipeline: Option<wgpu::RenderPipeline>,
}

impl PbrNode {
    pub fn init_pipeline(&mut self, renderer: &Renderer) {
        let device = renderer.device();
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR Pipeline Layout"),
            bind_group_layouts: &[
                &GpuMaterial::bind_group_layout(device),
                &GpuCamera::bind_group_layout(device),
                &GpuTransform::bind_group_layout(device),
                &GpuPointLightArray::bind_group_layout(device),
            ],
            push_constant_ranges: &[],
        });

        let shader = Shader::new(renderer.device(), "assets/shaders/pbr.wgsl");

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBR Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader.module,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 4 * (3 + 3 + 3 + 2) as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 4 * 3,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 4 * 6,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 4 * 9,
                            shader_location: 3,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader.module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            multiview: None,
        });

        self.pipeline = Some(pipeline);
    }
}

impl Render for PbrNode {
    fn prepare(&mut self, _world: &World, renderer: &Renderer, _entity: Entity) -> Result<()> {
        if self.pipeline.is_none() {
            self.init_pipeline(renderer);
        }

        Ok(())
    }

    fn render(
        &self,
        world: &World,
        renderer: &Renderer,
        input_slots: &[Slot],
    ) -> Result<Vec<Slot>> {
        log::trace!("PbrNode::render");
        let query = world.query(
            &Query::new()
                .read::<Handle<GpuMesh>>()
                .read::<BindGroup<GpuMaterial>>(),
        );

        let Slot::Texture(color_target) = &input_slots[0] else {
            bail!("PbrNode expected a texture in slot 0");
        };

        let Slot::Texture(depth_target) = &input_slots[1] else {
            bail!("PbrNode expected a texture in slot 1");
        };

        let Slot::BindGroup(camera_bind_group) = &input_slots[2] else {
            bail!("PbrNode expected a bind group in slot 2");
        };

        let Slot::BindGroup(light_bind_group) = &input_slots[3] else {
            bail!("PbrNode expected a bind group in slot 3");
        };

        let pipeline = self.pipeline.as_ref().unwrap();

        let mut encoder =
            renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("PBR Command Encoder"),
                });

        for entity in query.iter() {
            let assets = world.get_resource::<Assets>().unwrap();
            let mesh = query.get::<Handle<GpuMesh>>(entity).unwrap();
            let mesh = assets.get::<GpuMesh>(*mesh).unwrap();
            let material_bind_group = query.get::<BindGroup<GpuMaterial>>(entity).unwrap();
            let transform_bind_group = query.get::<BindGroup<GpuTransform>>(entity).unwrap();

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("PBR Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color_target,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_target,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(0, &material_bind_group, &[]);
                render_pass.set_bind_group(1, camera_bind_group, &[]);
                render_pass.set_bind_group(2, &transform_bind_group, &[]);
                render_pass.set_bind_group(3, light_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }
        }

        renderer.enqueue_command_buffer(encoder.finish());

        Ok(vec![
            Slot::Texture(color_target.clone()),
            Slot::Texture(depth_target.clone()),
        ])
    }
}
