use std::collections::HashMap;

use weaver_asset::{Assets, Handle};
use weaver_core::{prelude::Mat4, transform::Transform};
use weaver_ecs::{entity::Entity, prelude::World};
use weaver_renderer::{
    bind_group::{ComponentBindGroup, CreateComponentBindGroup, CreateResourceBindGroup},
    camera::GpuCamera,
    graph::{Render, Slot},
    mesh::GpuMesh,
    prelude::*,
    shader::Shader,
    texture::format::{DEPTH_FORMAT, VIEW_FORMAT},
    WgpuDevice, WgpuQueue,
};
use weaver_util::{
    lock::Lock,
    prelude::{bail, Result},
};

use crate::{light::GpuPointLightArray, material::GpuMaterial};

struct UniqueMaterialMesh {
    material: Handle<ComponentBindGroup<GpuMaterial>>,
    mesh: Handle<GpuMesh>,
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
    entities: Vec<Entity>,
}

pub struct PbrNode {
    #[allow(unused)]
    camera_entity: Entity, // todo: use for culling

    pipeline: Lock<Option<wgpu::RenderPipeline>>,

    #[allow(clippy::type_complexity)]
    unique_material_meshes: Lock<
        HashMap<(Handle<ComponentBindGroup<GpuMaterial>>, Handle<GpuMesh>), UniqueMaterialMesh>,
    >,

    transform_bind_group_layout: Lock<Option<wgpu::BindGroupLayout>>,
}

impl PbrNode {
    pub fn new(camera_entity: Entity) -> Self {
        Self {
            camera_entity,
            pipeline: Lock::new(None),
            unique_material_meshes: Lock::new(HashMap::new()),
            transform_bind_group_layout: Lock::new(None),
        }
    }

    pub fn init_pipeline(&self, render_world: &mut World) {
        let device = render_world.get_resource::<WgpuDevice>().unwrap();

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
                &GpuMaterial::bind_group_layout(&device),
                &GpuCamera::bind_group_layout(&device),
                &transform_bind_group_layout,
                &GpuPointLightArray::bind_group_layout(&device),
            ],
            push_constant_ranges: &[],
        });

        *self.transform_bind_group_layout.write() = Some(transform_bind_group_layout);

        let shader = Shader::new(&device, "assets/shaders/pbr.wgsl");

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

        *self.pipeline.write() = Some(pipeline);
    }
}

impl Render for PbrNode {
    fn prepare(&self, render_world: &mut World) -> Result<()> {
        if self.pipeline.read().is_none() {
            self.init_pipeline(render_world);
        }

        for unique_material_mesh in self.unique_material_meshes.write().values_mut() {
            unique_material_mesh.entities.clear();
        }

        let query =
            render_world.query::<(&Handle<ComponentBindGroup<GpuMaterial>>, &Handle<GpuMesh>)>();

        for (entity, (material, gpu_mesh)) in query.iter() {
            let mut unique_material_meshes = self.unique_material_meshes.write();

            let device = render_world.get_resource::<WgpuDevice>().unwrap();

            let unique_material_mesh = unique_material_meshes
                .entry((*material, *gpu_mesh))
                .or_insert_with(|| {
                    let transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("PBR Transform Buffer"),
                        size: std::mem::size_of::<Mat4>() as u64 * 4096,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
                        mapped_at_creation: false,
                    });

                    let transform_bind_group =
                        device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("PBR Transform Bind Group"),
                            layout: self.transform_bind_group_layout.read().as_ref().unwrap(),
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
                        transform_buffer,
                        transform_bind_group,
                        entities: Vec::new(),
                    }
                });

            unique_material_mesh.entities.push(entity);
        }

        for unique_material_mesh in self.unique_material_meshes.read().values() {
            let UniqueMaterialMesh {
                transform_buffer,
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

            queue.write_buffer(transform_buffer, 0, bytemuck::cast_slice(&transforms));
        }

        Ok(())
    }

    fn render(&self, render_world: &mut World, input_slots: &[Slot]) -> Result<Vec<Slot>> {
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

        let pipeline = self.pipeline.read();
        let pipeline = pipeline.as_ref().unwrap();

        log::trace!("PbrNode::render");

        let device = render_world.get_resource::<WgpuDevice>().unwrap();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("PBR Command Encoder"),
        });

        for unique_material_mesh in self.unique_material_meshes.read().values() {
            let assets = render_world.get_resource::<Assets>().unwrap();

            let UniqueMaterialMesh {
                material,
                mesh,
                transform_buffer: _,
                transform_bind_group,
                entities,
            } = unique_material_mesh;

            let material_bind_group = assets
                .get::<ComponentBindGroup<GpuMaterial>>(*material)
                .unwrap();
            let mesh = assets.get::<GpuMesh>(*mesh).unwrap();

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
                render_pass.set_bind_group(2, transform_bind_group, &[]);
                render_pass.set_bind_group(3, light_bind_group, &[]);

                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.num_indices, 0, 0..entities.len() as u32);
            }
        }

        let mut renderer = render_world.get_resource_mut::<Renderer>().unwrap();

        renderer.enqueue_command_buffer(encoder.finish());

        Ok(vec![
            Slot::Texture(color_target.clone()),
            Slot::Texture(depth_target.clone()),
        ])
    }
}
