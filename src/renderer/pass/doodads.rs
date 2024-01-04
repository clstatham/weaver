use wgpu::util::DeviceExt;

use crate::{
    core::{color::Color, doodads::Doodad, texture::Texture},
    include_shader,
};

use super::Pass;

#[rustfmt::skip]
const CUBE_VERTICES: &[glam::Vec3] = &[
    // front
    glam::Vec3::new(-0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, 0.5),
    glam::Vec3::new(-0.5, 0.5, 0.5),
    // back
    glam::Vec3::new(-0.5, -0.5, -0.5),
    glam::Vec3::new(0.5, -0.5, -0.5),
    glam::Vec3::new(0.5, 0.5, -0.5),
    glam::Vec3::new(-0.5, 0.5, -0.5),
    // top
    glam::Vec3::new(-0.5, 0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, -0.5),
    glam::Vec3::new(-0.5, 0.5, -0.5),
    // bottom
    glam::Vec3::new(-0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, -0.5, -0.5),
    glam::Vec3::new(-0.5, -0.5, -0.5),
    // left
    glam::Vec3::new(-0.5, -0.5, 0.5),
    glam::Vec3::new(-0.5, 0.5, 0.5),
    glam::Vec3::new(-0.5, 0.5, -0.5),
    glam::Vec3::new(-0.5, -0.5, -0.5),
    // right
    glam::Vec3::new(0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, -0.5),
    glam::Vec3::new(0.5, -0.5, -0.5),
];

#[rustfmt::skip]
const CUBE_NORMALS: &[glam::Vec3] = &[
    // front
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    // back
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    // top
    glam::Vec3::new(0.0, 1.0, 0.0),
    glam::Vec3::new(0.0, 1.0, 0.0),
    glam::Vec3::new(0.0, 1.0, 0.0),
    glam::Vec3::new(0.0, 1.0, 0.0),
    // bottom
    glam::Vec3::new(0.0, -1.0, 0.0),
    glam::Vec3::new(0.0, -1.0, 0.0),
    glam::Vec3::new(0.0, -1.0, 0.0),
    glam::Vec3::new(0.0, -1.0, 0.0),
    // left
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    // right
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
];

#[rustfmt::skip]
const CUBE_INDICES: &[u32] = &[
    // front
    0, 1, 2, 2, 3, 0,
    // back
    4, 5, 6, 6, 7, 4,
    // top
    8, 9, 10, 10, 11, 8,
    // bottom
    12, 13, 14, 14, 15, 12,
    // left
    16, 17, 18, 18, 19, 16,
    // right
    20, 21, 22, 22, 23, 20,
];

fn cube_vertices() -> Vec<DoodadVertex> {
    let mut vertices = Vec::new();

    for (position, normal) in CUBE_VERTICES.iter().zip(CUBE_NORMALS.iter()) {
        vertices.push(DoodadVertex {
            position: *position,
            normal: *normal,
        });
    }

    vertices
}

fn cone_vertices(n: usize) -> Vec<DoodadVertex> {
    let mut vertices = Vec::new();

    let mut positions = Vec::new();
    let mut normals = Vec::new();

    for i in 0..n {
        let theta = 2.0 * std::f32::consts::PI * (i as f32 / n as f32);
        let x = theta.cos();
        let y = theta.sin();

        positions.push(glam::Vec3::new(x, y, 0.0));
        normals.push(glam::Vec3::new(0.0, 0.0, 1.0));
    }

    positions.push(glam::Vec3::new(0.0, 0.0, 1.0));
    normals.push(glam::Vec3::new(0.0, 0.0, 1.0));

    for (position, normal) in positions.iter().zip(normals.iter()) {
        vertices.push(DoodadVertex {
            position: *position,
            normal: *normal,
        });
    }

    vertices
}

fn cone_indices(n: usize) -> Vec<u32> {
    let mut indices = Vec::new();

    for i in 0..n {
        let i0 = i;
        let i1 = (i + 1) % n;
        let i2 = n;

        indices.push(i0 as u32);
        indices.push(i1 as u32);
        indices.push(i2 as u32);
    }

    indices
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct DoodadVertex {
    position: glam::Vec3,
    normal: glam::Vec3,
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct DoodadCamera {
    view: glam::Mat4,
    proj: glam::Mat4,
}

#[allow(dead_code)]
pub struct DoodadRenderPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    depth_texture: Texture,
    camera_buffer: wgpu::Buffer,

    cube_vertex_buffer: wgpu::Buffer,
    cube_index_buffer: wgpu::Buffer,
    cone_vertex_buffer: wgpu::Buffer,
    cone_index_buffer: wgpu::Buffer,

    model_transform_buffer: wgpu::Buffer,
    doodad_color_buffer: wgpu::Buffer,
}

impl DoodadRenderPass {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let depth_texture = Texture::create_depth_texture(
            device,
            config.width,
            config.height,
            Some("doodad depth texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );

        let model_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doodad model transform buffer"),
            size: std::mem::size_of::<glam::Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doodad camera buffer"),
            size: std::mem::size_of::<DoodadCamera>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let doodad_color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doodad color buffer"),
            size: std::mem::size_of::<Color>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cube_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("doodad cube vertex buffer"),
            contents: bytemuck::cast_slice(&cube_vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cube_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("doodad cube index buffer"),
            contents: bytemuck::cast_slice(CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let cone_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("doodad cone vertex buffer"),
            contents: bytemuck::cast_slice(&cone_vertices(32)),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cone_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("doodad cone index buffer"),
            contents: bytemuck::cast_slice(&cone_indices(32)),
            usage: wgpu::BufferUsages::INDEX,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("doodad bind group layout"),
            entries: &[
                // model_transform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // camera
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // color
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("doodad pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("doodad shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("doodads.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("doodad pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<DoodadVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3,
                        1 => Float32x3,
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: Texture::HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("doodad bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_transform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: doodad_color_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            bind_group,
            bind_group_layout,
            depth_texture,
            camera_buffer,
            cube_vertex_buffer,
            cube_index_buffer,
            cone_vertex_buffer,
            cone_index_buffer,
            model_transform_buffer,
            doodad_color_buffer,
        }
    }
}

impl Pass for DoodadRenderPass {
    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        _depth_target: &Texture,
        world: &crate::ecs::World,
    ) -> anyhow::Result<()> {
        let mut doodads = world.write_resource::<crate::core::doodads::Doodads>()?;
        let doodads = doodads.doodads.drain(..).collect::<Vec<_>>();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("doodad encoder"),
        });

        let camera = world.read_resource::<crate::core::camera::FlyCamera>()?;
        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[DoodadCamera {
                view: camera.view_matrix(),
                proj: camera.projection_matrix(),
            }]),
        );

        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("doodad render pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.depth_texture.view(),
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

        queue.submit(std::iter::once(encoder.finish()));

        for doodad in doodads {
            match doodad {
                Doodad::Cube(cube) => {
                    let transform = glam::Mat4::from_scale_rotation_translation(
                        cube.scale,
                        cube.rotation,
                        cube.position,
                    );

                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("doodad encoder"),
                        });

                    queue.write_buffer(
                        &self.model_transform_buffer,
                        0,
                        bytemuck::cast_slice(&[transform]),
                    );

                    queue.write_buffer(
                        &self.doodad_color_buffer,
                        0,
                        bytemuck::cast_slice(&[cube.color]),
                    );

                    {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("doodad render pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: color_target.view(),
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: self.depth_texture.view(),
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Load,
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        stencil_ops: None,
                                    },
                                ),
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });

                        render_pass.set_pipeline(&self.pipeline);
                        render_pass.set_bind_group(0, &self.bind_group, &[]);
                        render_pass.set_index_buffer(
                            self.cube_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        render_pass.set_vertex_buffer(0, self.cube_vertex_buffer.slice(..));
                        render_pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..1);
                    }

                    queue.submit(std::iter::once(encoder.finish()));
                }
                Doodad::Cone(cone) => {
                    let transform = glam::Mat4::from_scale_rotation_translation(
                        cone.scale,
                        cone.rotation,
                        cone.position,
                    );

                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("doodad encoder"),
                        });

                    queue.write_buffer(
                        &self.model_transform_buffer,
                        0,
                        bytemuck::cast_slice(&[transform]),
                    );

                    queue.write_buffer(
                        &self.doodad_color_buffer,
                        0,
                        bytemuck::cast_slice(&[cone.color]),
                    );

                    {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("doodad render pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: color_target.view(),
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: self.depth_texture.view(),
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Load,
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        stencil_ops: None,
                                    },
                                ),
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });

                        render_pass.set_pipeline(&self.pipeline);
                        render_pass.set_bind_group(0, &self.bind_group, &[]);
                        render_pass.set_index_buffer(
                            self.cone_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        render_pass.set_vertex_buffer(0, self.cone_vertex_buffer.slice(..));
                        render_pass.draw_indexed(0..cone_indices(32).len() as u32, 0, 0..1);
                    }

                    queue.submit(std::iter::once(encoder.finish()));
                }
            }
        }

        Ok(())
    }
}
