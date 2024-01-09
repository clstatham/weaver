use std::cell::RefCell;

use crate::{
    core::{
        camera::Camera,
        material::Material,
        mesh::Vertex,
        texture::{DepthFormat, HdrFormat, NormalMapFormat, PositionMapFormat, TextureFormat},
        transform::Transform,
    },
    ecs::{Query, World},
    include_shader,
    renderer::{AllocBuffers, BindGroupLayoutCache, Renderer},
};

use super::{
    pbr::{UniqueMesh, UniqueMeshes},
    Pass,
};

pub struct GBufferRenderPass {
    pipeline: wgpu::RenderPipeline,
    unique_meshes: RefCell<UniqueMeshes>,
}

impl GBufferRenderPass {
    pub fn new(device: &wgpu::Device, layout_cache: &BindGroupLayoutCache) -> Self {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GBuffer Pipeline Layout"),
            bind_group_layouts: &[
                // Mesh Transform
                &layout_cache.get_or_create::<Transform>(device),
                // Camera
                &layout_cache.get_or_create::<Camera>(device),
                // Material
                &layout_cache.get_or_create::<Material>(device),
            ],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GBuffer Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("gbuffer.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GBuffer Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[
                    // color target
                    Some(wgpu::ColorTargetState {
                        format: HdrFormat::FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    // normal target
                    Some(wgpu::ColorTargetState {
                        format: NormalMapFormat::FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    // position target
                    Some(wgpu::ColorTargetState {
                        format: PositionMapFormat::FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthFormat::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            pipeline,
            unique_meshes: RefCell::new(UniqueMeshes::default()),
        }
    }
}

impl Pass for GBufferRenderPass {
    fn is_enabled(&self) -> bool {
        true
    }
    fn disable(&mut self) {}
    fn enable(&mut self) {}

    fn prepare(&self, world: &World, renderer: &Renderer) -> anyhow::Result<()> {
        let mut unique_meshes = self.unique_meshes.borrow_mut();
        unique_meshes.gather(world, renderer);
        unique_meshes.alloc_buffers(renderer)?;
        unique_meshes.update();

        Ok(())
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        renderer: &Renderer,
        world: &World,
    ) -> anyhow::Result<()> {
        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();
        let camera_handle = &camera.alloc_buffers(renderer)?[0];
        let camera_bind_group = camera_handle.bind_group().unwrap();

        let unique_meshes = self.unique_meshes.borrow();

        for mesh in unique_meshes.unique_meshes.values() {
            let UniqueMesh {
                mesh,
                material_bind_group,
                transforms,
                transform_buffer,
            } = mesh;

            let transform_handle = transform_buffer.get_or_create::<Transform>(renderer);
            let transform_bind_group = transform_handle.bind_group().unwrap();

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PBR Render Pass"),
                color_attachments: &[
                    // color target
                    Some(wgpu::RenderPassColorAttachment {
                        view: color_target,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    // normal target
                    Some(wgpu::RenderPassColorAttachment {
                        view: &renderer.normal_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    // position target
                    Some(wgpu::RenderPassColorAttachment {
                        view: &renderer.position_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_target,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &transform_bind_group, &[]);
            render_pass.set_bind_group(1, &camera_bind_group, &[]);
            render_pass.set_bind_group(2, material_bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
            render_pass.set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.num_indices() as u32, 0, 0..transforms.len() as u32);
        }

        Ok(())
    }
}
