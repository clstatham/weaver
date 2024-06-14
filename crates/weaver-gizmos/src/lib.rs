#![allow(unused)] // todo: remove

use std::{path::Path, sync::Arc};

use weaver_app::{plugin::Plugin, system::SystemStage, App};
use weaver_asset::{prelude::Asset, Assets};
use weaver_core::{color::Color, transform::Transform};
use weaver_ecs::{prelude::Resource, query::Query, world::World};
use weaver_pbr::render::PbrNode;
use weaver_renderer::{
    asset::RenderAsset,
    buffer::GpuBuffer,
    mesh::primitive::{CubePrimitive, Primitive},
    prelude::*,
    shader::Shader,
    texture::format::VIEW_FORMAT,
};
use weaver_util::{
    lock::Lock,
    prelude::{anyhow, impl_downcast, DowncastSync, Result},
};
use wgpu::util::DeviceExt;

pub trait Gizmo: Asset {}

pub struct CubeGizmo;
impl Asset for CubeGizmo {
    fn load(_assets: &mut Assets, _path: &std::path::Path) -> Result<Self>
    where
        Self: Sized,
    {
        Err(anyhow!("CubeGizmo is not loadable"))
    }
}
impl Gizmo for CubeGizmo {}

pub struct RenderCubeGizmo {
    pub vertex_buffer: GpuBuffer,
    pub index_buffer: GpuBuffer,
}

impl Asset for RenderCubeGizmo {
    fn load(_assets: &mut Assets, _path: &Path) -> Result<Self>
    where
        Self: Sized,
    {
        Err(anyhow!("RenderCubeGizmo is not loadable"))
    }
}

impl RenderAsset for RenderCubeGizmo {
    type BaseAsset = CubeGizmo;

    fn extract_render_asset(
        _base_asset: &Self::BaseAsset,
        _world: &World,
        renderer: &Renderer,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let device = renderer.device();

        let cube = CubePrimitive::new(1.0);
        let mesh = cube.generate_mesh();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CubeVertexBuffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CubeIndexBuffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Some(Self {
            vertex_buffer: GpuBuffer::new(vertex_buffer),
            index_buffer: GpuBuffer::new(index_buffer),
        })
    }

    fn update_render_asset(
        &self,
        _base_asset: &Self::BaseAsset,
        _world: &World,
        _renderer: &Renderer,
    ) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GizmoMode {
    Solid,
    Wireframe,
}

pub struct GizmoInstance {
    pub gizmo: Arc<dyn Gizmo>,
    pub mode: GizmoMode,
    pub depth_test: bool,
    pub color: Color,
    pub transform: Transform,
}

impl GizmoInstance {
    pub fn new<T: Gizmo>(
        gizmo: T,
        mode: GizmoMode,
        depth_test: bool,
        color: Color,
        transform: Transform,
    ) -> Self {
        Self {
            gizmo: Arc::new(gizmo),
            mode,
            depth_test,
            color,
            transform,
        }
    }
}

#[derive(Resource)]
pub struct Gizmos {
    pub(crate) gizmos: Lock<Vec<GizmoInstance>>,
}

impl Default for Gizmos {
    fn default() -> Self {
        Self {
            gizmos: Lock::new(Vec::new()),
        }
    }
}

struct GizmoRenderNodeInner {
    pipeline: wgpu::RenderPipeline,
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
    transform_bind_group_layout: wgpu::BindGroupLayout,
    color_buffer: wgpu::Buffer,
    color_bind_group: wgpu::BindGroup,
    color_bind_group_layout: wgpu::BindGroupLayout,
}

pub struct GizmoRenderNode {
    inner: Lock<Option<GizmoRenderNodeInner>>,
}

impl GizmoRenderNode {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inner: Lock::new(None),
        }
    }

    pub fn init_pipeline(&self, renderer: &Renderer) -> Result<()> {
        let device = renderer.device();

        let transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GizmoTransformBuffer"),
            size: std::mem::size_of::<Transform>() as u64 * 1024,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("GizmoTransformBindGroupLayout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GizmoColorBuffer"),
            size: std::mem::size_of::<Color>() as u64 * 1024,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let color_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("GizmoColorBindGroupLayout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GizmoPipelineLayout"),
            bind_group_layouts: &[&transform_bind_group_layout, &color_bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = Shader::new(device, "assets/shaders/gizmos.wgsl");
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GizmoRenderNode"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader.module,
                entry_point: "main",
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
                entry_point: "main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: VIEW_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GizmoTransformBindGroup"),
            layout: &transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: transform_buffer.as_entire_binding(),
            }],
        });

        let color_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GizmoColorBindGroup"),
            layout: &color_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: color_buffer.as_entire_binding(),
            }],
        });

        *self.inner.write() = Some(GizmoRenderNodeInner {
            pipeline,
            transform_buffer,
            transform_bind_group,
            transform_bind_group_layout,
            color_buffer,
            color_bind_group,
            color_bind_group_layout,
        });

        Ok(())
    }
}

impl Render for GizmoRenderNode {
    fn prepare(&self, world: &Arc<World>, renderer: &Renderer) -> Result<()> {
        if self.inner.read().is_none() {
            self.init_pipeline(renderer)?;
        }

        let Some(gizmos) = world.get_resource::<Gizmos>() else {
            return Ok(());
        };

        let Some(inner) = &*self.inner.read() else {
            return Ok(());
        };

        let mut transform_data: Vec<u8> = Vec::new();
        let mut color_data: Vec<u8> = Vec::new();

        for gizmo in gizmos.gizmos.read().iter() {
            let transform = gizmo.transform;
            transform_data.extend_from_slice(bytemuck::cast_slice(&[transform.matrix()]));

            let color = gizmo.color;
            color_data.extend_from_slice(bytemuck::cast_slice(&[color]));
        }

        let queue = renderer.queue();

        queue.write_buffer(&inner.transform_buffer, 0, &transform_data);
        queue.write_buffer(&inner.color_buffer, 0, &color_data);

        Ok(())
    }

    fn render(
        &self,
        world: &Arc<World>,
        renderer: &Renderer,
        input_slots: &[Slot],
    ) -> Result<Vec<Slot>> {
        let Some(gizmos) = world.get_resource::<Gizmos>() else {
            return Ok(input_slots.to_vec());
        };

        let Some(inner) = &*self.inner.read() else {
            return Ok(input_slots.to_vec());
        };

        Ok(input_slots.to_vec())
    }
}

pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.insert_resource(Gizmos::default());
        app.add_system(inject_gizmo_render_node, SystemStage::PreRender)?;

        Ok(())
    }
}

fn inject_gizmo_render_node(cameras: Query<&mut Camera>) -> Result<()> {
    for (_, mut camera) in cameras.iter() {
        if camera.render_graph().has_node::<GizmoRenderNode>() {
            continue;
        }

        if let Some(pbr_node) = camera.render_graph().node_index::<PbrNode>() {
            let gizmo_node = camera
                .render_graph_mut()
                .add_node(RenderNode::new("GizmoRenderNode", GizmoRenderNode::new()));

            camera
                .render_graph_mut()
                .add_edge(pbr_node, 0, gizmo_node, 0);
            camera
                .render_graph_mut()
                .add_edge(pbr_node, 1, gizmo_node, 1);
        }
    }

    Ok(())
}
