use weaver_app::{plugin::Plugin, App, PrepareFrame};
use weaver_core::{color::Color, transform::Transform};
use weaver_ecs::{component::Res, prelude::Resource, query::Query, world::World};
use weaver_pbr::{
    camera::{CameraRenderComponent, PbrCameraBindGroupNode},
    render::PbrNode,
};
use weaver_renderer::{
    bind_group::CreateComponentBindGroup,
    buffer::GpuBuffer,
    camera::GpuCamera,
    extract::{RenderResource, RenderResourcePlugin},
    mesh::primitive::{CubePrimitive, Primitive},
    prelude::*,
    shader::Shader,
    texture::format::VIEW_FORMAT,
    PreRender, RenderApp, WgpuDevice, WgpuQueue,
};
use weaver_util::{
    lock::{Lock, SharedLock},
    prelude::{anyhow, Result},
};

use wgpu::util::DeviceExt;

pub mod prelude {
    pub use super::{Gizmo, GizmoMode, GizmoPlugin, Gizmos};
}

pub enum Gizmo {
    Cube,
}

#[derive(Resource)]
pub struct RenderCubeGizmo {
    pub vertex_buffer: GpuBuffer,
    pub index_buffer: GpuBuffer,
    pub num_indices: usize,
}

impl RenderResource for RenderCubeGizmo {
    fn extract_render_resource(_main_world: &mut World, render_world: &mut World) -> Option<Self>
    where
        Self: Sized,
    {
        let device = render_world.get_resource::<WgpuDevice>().unwrap();

        let cube = CubePrimitive::new(1.0, true);
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
            num_indices: mesh.indices.len(),
        })
    }

    fn update_render_resource(
        &mut self,
        _main_world: &mut World,
        _render_world: &mut World,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub enum GizmoMode {
    Solid,
    Wireframe,
}

pub struct GizmoInstance {
    pub gizmo: Gizmo,
    pub mode: GizmoMode,
    pub depth_test: bool,
    pub color: Color,
    pub transform: Transform,
}

impl GizmoInstance {
    pub fn new(
        gizmo: Gizmo,
        mode: GizmoMode,
        depth_test: bool,
        color: Color,
        transform: Transform,
    ) -> Self {
        Self {
            gizmo,
            mode,
            depth_test,
            color,
            transform,
        }
    }
}

#[derive(Resource, Clone)]
pub struct Gizmos {
    pub(crate) gizmos: SharedLock<Vec<GizmoInstance>>,
}

impl Default for Gizmos {
    fn default() -> Self {
        Self {
            gizmos: SharedLock::new(Vec::new()),
        }
    }
}

impl Gizmos {
    pub fn add_gizmo(&self, gizmo: GizmoInstance) {
        self.gizmos.write().push(gizmo);
    }

    pub fn clear(&self) {
        self.gizmos.write().clear();
    }

    pub fn cube(&self, transform: Transform, color: Color) {
        self.add_gizmo(GizmoInstance::new(
            Gizmo::Cube,
            GizmoMode::Solid,
            true,
            color,
            transform,
        ));
    }
}

struct GizmoRenderNodeInner {
    pipeline: wgpu::RenderPipeline,
    transform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    color_buffer: wgpu::Buffer,
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

    pub fn init_pipeline(&self, render_world: &mut World) -> Result<()> {
        let device = render_world.get_resource::<WgpuDevice>().unwrap();

        let transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GizmoTransformBuffer"),
            size: std::mem::size_of::<Transform>() as u64 * 1024,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GizmoColorBuffer"),
            size: std::mem::size_of::<Color>() as u64 * 1024,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GizmoTransformBindGroupLayout"),
            entries: &[
                // transform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // color
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GizmoPipelineLayout"),
            bind_group_layouts: &[&bind_group_layout, &GpuCamera::bind_group_layout(&device)],
            push_constant_ranges: &[],
        });

        let shader = Shader::new(&device, "assets/shaders/gizmos.wgsl");
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GizmoRenderNode"),
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
                topology: wgpu::PrimitiveTopology::LineList,
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GizmoTransformBindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: transform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: color_buffer.as_entire_binding(),
                },
            ],
        });

        *self.inner.write() = Some(GizmoRenderNodeInner {
            pipeline,
            transform_buffer,
            bind_group,
            color_buffer,
        });

        Ok(())
    }
}

impl Render for GizmoRenderNode {
    fn prepare(&self, render_world: &mut World) -> Result<()> {
        if self.inner.read().is_none() {
            self.init_pipeline(render_world)?;
        }

        let Some(gizmos) = render_world.get_resource::<Gizmos>() else {
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

        let queue = render_world.get_resource::<WgpuQueue>().unwrap();

        queue.write_buffer(&inner.transform_buffer, 0, &transform_data);
        queue.write_buffer(&inner.color_buffer, 0, &color_data);

        Ok(())
    }

    fn render(&self, render_world: &mut World, input_slots: &[Slot]) -> Result<Vec<Slot>> {
        let Some(gizmos) = render_world.get_resource::<Gizmos>() else {
            return Ok(input_slots.to_vec());
        };

        let Some(inner) = &*self.inner.read() else {
            return Ok(input_slots.to_vec());
        };

        let Slot::Texture(color_target) = &input_slots[0] else {
            return Err(anyhow!("Expected a texture slot"));
        };

        let Slot::Texture(depth_target) = &input_slots[1] else {
            return Err(anyhow!("Expected a texture slot"));
        };

        let Slot::BindGroup(camera_bind_group) = &input_slots[2] else {
            return Err(anyhow!("Expected a bind group slot"));
        };

        let cube_resource = render_world.get_resource::<RenderCubeGizmo>().unwrap();

        let device = render_world.get_resource::<WgpuDevice>().unwrap();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("GizmoCommandEncoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GizmoRenderPass"),
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

            render_pass.set_pipeline(&inner.pipeline);
            render_pass.set_bind_group(0, &inner.bind_group, &[]);
            render_pass.set_bind_group(1, camera_bind_group, &[]);

            for (i, gizmo) in gizmos.gizmos.read().iter().enumerate() {
                match gizmo.gizmo {
                    Gizmo::Cube => {
                        render_pass.set_vertex_buffer(0, cube_resource.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(
                            cube_resource.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        render_pass.draw_indexed(
                            0..cube_resource.num_indices as u32,
                            0,
                            (i as u32)..(i as u32 + 1),
                        );
                    }
                }
            }
        }

        let mut renderer = render_world.get_resource_mut::<Renderer>().unwrap();

        renderer.enqueue_command_buffer(encoder.finish());

        Ok(input_slots.to_vec())
    }
}

pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        let gizmos = Gizmos::default();
        app.insert_resource(gizmos.clone());
        app.add_system(clear_gizmos, PrepareFrame);

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.insert_resource(gizmos);
        render_app.add_plugin(RenderResourcePlugin::<RenderCubeGizmo>::default())?;
        render_app.add_system(inject_gizmo_render_node, PreRender);

        Ok(())
    }
}

fn inject_gizmo_render_node(cameras: Query<&mut CameraRenderComponent>) -> Result<()> {
    for (_, mut camera) in cameras.iter() {
        if camera.graph.has_node::<GizmoRenderNode>() {
            continue;
        }

        if let Some(pbr_node) = camera.graph.node_index::<PbrNode>() {
            let gizmo_node = camera
                .graph
                .add_node(RenderNode::new("GizmoRenderNode", GizmoRenderNode::new()));
            let camera_node = camera.graph.node_index::<PbrCameraBindGroupNode>().unwrap();

            camera.graph.add_edge(pbr_node, 0, gizmo_node, 0);
            camera.graph.add_edge(pbr_node, 1, gizmo_node, 1);
            camera.graph.add_edge(camera_node, 0, gizmo_node, 2);
        }
    }

    Ok(())
}

fn clear_gizmos(gizmos: Res<Gizmos>) -> Result<()> {
    gizmos.clear();
    Ok(())
}
