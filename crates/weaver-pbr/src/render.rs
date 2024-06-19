use std::{collections::HashMap, path::Path};

use weaver_asset::{Assets, Handle};
use weaver_core::{prelude::Mat4, transform::Transform};
use weaver_ecs::{entity::Entity, prelude::World, storage::Ref};
use weaver_renderer::{
    bind_group::{BindGroup, BindGroupLayout, BindGroupLayoutCache, CreateBindGroup},
    buffer::GpuBuffer,
    camera::{GpuCamera, ViewTarget},
    graph::{RenderCtx, RenderGraphCtx, ViewNode},
    mesh::GpuMesh,
    pipeline::{CreateRenderPipeline, RenderPipeline, RenderPipelineCache, RenderPipelineLayout},
    prelude::*,
    shader::Shader,
    texture::format::{DEPTH_FORMAT, VIEW_FORMAT},
    RenderLabel, WgpuDevice, WgpuQueue,
};
use weaver_util::{lock::Lock, prelude::Result};

use crate::{light::GpuPointLightArray, material::GpuMaterial};

pub struct TransformArray {
    pub buffer: GpuBuffer,
    pub bind_group: wgpu::BindGroup,
}

impl TransformArray {
    pub fn new(buffer: GpuBuffer, bind_group: wgpu::BindGroup) -> Self {
        Self { buffer, bind_group }
    }
}

impl CreateBindGroup for TransformArray {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Transform Array Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Transform Array Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        })
    }
}

struct UniqueMaterialMesh {
    material: Handle<BindGroup<GpuMaterial>>,
    mesh: Handle<GpuMesh>,
    transforms: TransformArray,
    entities: Vec<Entity>,
}

#[derive(Debug, Clone, Copy)]
pub struct PbrNodeLabel;
impl RenderLabel for PbrNodeLabel {}

pub struct PbrNode {
    #[allow(clippy::type_complexity)]
    unique_material_meshes:
        Lock<HashMap<(Handle<BindGroup<GpuMaterial>>, Handle<GpuMesh>), UniqueMaterialMesh>>,
}

impl Default for PbrNode {
    fn default() -> Self {
        Self {
            unique_material_meshes: Lock::new(HashMap::new()),
        }
    }
}

impl PbrNode {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CreateRenderPipeline for PbrNode {
    fn create_render_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> RenderPipelineLayout
    where
        Self: Sized,
    {
        let transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("PBR Transform Storage Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR Pipeline Layout"),
            bind_group_layouts: &[
                &bind_group_layout_cache.get_or_create::<GpuMaterial>(device),
                &bind_group_layout_cache.get_or_create::<GpuCamera>(device),
                &transform_bind_group_layout,
                &bind_group_layout_cache.get_or_create::<GpuPointLightArray>(device),
            ],
            push_constant_ranges: &[],
        });

        RenderPipelineLayout::new(layout)
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        cached_layout: &wgpu::PipelineLayout,
    ) -> RenderPipeline
    where
        Self: Sized,
    {
        let shader = Shader::new(Path::new("assets/shaders/pbr.wgsl")).create_shader_module(device);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBR Pipeline"),
            layout: Some(cached_layout),
            vertex: wgpu::VertexState {
                module: &shader,
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
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: VIEW_FORMAT,
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
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            multiview: None,
        });

        RenderPipeline::new(pipeline)
    }
}

impl ViewNode for PbrNode {
    type ViewQueryFetch = &'static ViewTarget;
    type ViewQueryFilter = ();

    fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        let device = render_world.get_resource::<WgpuDevice>().unwrap();

        for unique_material_mesh in self.unique_material_meshes.write().values_mut() {
            unique_material_mesh.entities.clear();
        }

        let mut bind_group_layout_cache = render_world
            .get_resource_mut::<BindGroupLayoutCache>()
            .unwrap();

        let query = render_world.query::<(&Handle<BindGroup<GpuMaterial>>, &Handle<GpuMesh>)>();

        for (entity, (material, gpu_mesh)) in query.iter() {
            let mut unique_material_meshes = self.unique_material_meshes.write();

            let unique_material_mesh = unique_material_meshes
                .entry((*material, *gpu_mesh))
                .or_insert_with(|| {
                    let transform_buffer =
                        GpuBuffer::from(device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("PBR Transform Buffer"),
                            size: std::mem::size_of::<Mat4>() as u64 * 4096,
                            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
                            mapped_at_creation: false,
                        }));

                    let transform_bind_group =
                        device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("PBR Transform Bind Group"),
                            layout: &bind_group_layout_cache
                                .get_or_create::<TransformArray>(&device),
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: &transform_buffer,
                                    offset: 0,
                                    size: None,
                                }),
                            }],
                        });

                    UniqueMaterialMesh {
                        material: *material,
                        mesh: *gpu_mesh,
                        transforms: TransformArray::new(transform_buffer, transform_bind_group),
                        entities: Vec::new(),
                    }
                });

            unique_material_mesh.entities.push(entity);
        }

        for unique_material_mesh in self.unique_material_meshes.read().values() {
            let UniqueMaterialMesh {
                transforms: transform_buffer,
                entities,
                ..
            } = unique_material_mesh;

            let mut transforms = Vec::new();

            for entity in entities {
                let transform = render_world
                    .get_component::<Transform>(*entity)
                    .map(|t| *t)
                    .unwrap_or_default();
                transforms.push(transform.matrix());
            }

            let queue = render_world.get_resource::<WgpuQueue>().unwrap();

            queue.write_buffer(
                &transform_buffer.buffer,
                0,
                bytemuck::cast_slice(&transforms),
            );
        }

        let mut pipeline_cache = render_world
            .get_resource_mut::<RenderPipelineCache>()
            .unwrap();
        pipeline_cache.get_or_create_pipeline::<Self>(&device, &mut bind_group_layout_cache);

        Ok(())
    }

    fn run(
        &self,
        render_world: &World,
        graph_ctx: &mut RenderGraphCtx,
        render_ctx: &mut RenderCtx,
        view_target: &Ref<ViewTarget>,
    ) -> Result<()> {
        let light_bind_group = render_world
            .get_resource::<BindGroup<GpuPointLightArray>>()
            .unwrap();

        let camera_bind_group = render_world
            .get_component::<BindGroup<GpuCamera>>(graph_ctx.view_entity)
            .unwrap();

        log::trace!("PbrNode::render");

        let pipeline_cache = render_world.get_resource::<RenderPipelineCache>().unwrap();
        let pipeline = pipeline_cache.get_pipeline::<Self>().unwrap();

        for unique_material_mesh in self.unique_material_meshes.read().values() {
            let assets = render_world.get_resource::<Assets>().unwrap();

            let UniqueMaterialMesh {
                material,
                mesh,
                transforms,
                entities,
            } = unique_material_mesh;

            let material_bind_group = assets.get::<BindGroup<GpuMaterial>>(*material).unwrap();
            let mesh = assets.get::<GpuMesh>(*mesh).unwrap();

            {
                let mut render_pass = render_ctx.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(0, &material_bind_group, &[]);
                render_pass.set_bind_group(1, &camera_bind_group, &[]);
                render_pass.set_bind_group(2, &transforms.bind_group, &[]);
                render_pass.set_bind_group(3, &light_bind_group, &[]);

                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.num_indices, 0, 0..entities.len() as u32);
            }
        }

        Ok(())
    }
}
