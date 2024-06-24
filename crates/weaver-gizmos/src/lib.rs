use std::path::Path;

use weaver_app::{plugin::Plugin, App, PrepareFrame};
use weaver_core::{color::Color, prelude::Mat4, transform::Transform};
use weaver_ecs::{
    component::Res,
    prelude::Resource,
    query::QueryFetchItem,
    system::SystemParamItem,
    world::{FromWorld, World, WorldLock},
};
use weaver_pbr::render::PbrNodeLabel;
use weaver_renderer::{
    bind_group::{BindGroup, BindGroupLayout, BindGroupLayoutCache, CreateBindGroup},
    buffer::{GpuBuffer, GpuBufferVec},
    camera::{GpuCamera, ViewTarget},
    extract::{RenderResource, RenderResourcePlugin},
    graph::{RenderGraphApp, ViewNode, ViewNodeRunner},
    hdr::{HdrNodeLabel, HdrRenderTarget},
    mesh::primitive::{CubePrimitive, Primitive},
    pipeline::{
        CreateRenderPipeline, RenderPipeline, RenderPipelineCache, RenderPipelineLayout,
        RenderPipelinePlugin,
    },
    prelude::*,
    shader::Shader,
    texture::texture_format,
    RenderApp, RenderLabel, WgpuDevice, WgpuQueue,
};
use weaver_util::{lock::SharedLock, prelude::Result};

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
    type UpdateQuery = ();

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
            vertex_buffer: GpuBuffer::from(vertex_buffer),
            index_buffer: GpuBuffer::from(index_buffer),
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

#[derive(Debug, Clone, Copy)]
pub struct GizmoSubGraph;
impl RenderLabel for GizmoSubGraph {}

#[derive(Debug, Clone, Copy)]
pub struct GizmoNodeLabel;
impl RenderLabel for GizmoNodeLabel {}

pub struct GizmoRenderNode {
    transform_buffer: GpuBufferVec<Mat4>,
    color_buffer: GpuBufferVec<Color>,
    bind_group: Option<BindGroup<GizmoRenderNode>>,
}

impl GizmoRenderNode {
    #[allow(clippy::new_without_default)]
    pub fn new(device: &wgpu::Device) -> Self {
        let mut transform_buffer =
            GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        transform_buffer.reserve(1, device);
        let mut color_buffer =
            GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        color_buffer.reserve(1, device);

        Self {
            transform_buffer,
            color_buffer,
            bind_group: None,
        }
    }
}

impl FromWorld for GizmoRenderNode {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        Self::new(&device)
    }
}

impl CreateBindGroup for GizmoRenderNode {
    fn create_bind_group(
        &self,
        _render_world: &World,
        device: &wgpu::Device,
        cached_layout: &BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GizmoBindGroup"),
            layout: cached_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.transform_buffer.binding().unwrap(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.color_buffer.binding().unwrap(),
                },
            ],
        })
    }

    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        })
    }
}

impl CreateRenderPipeline for GizmoRenderNode {
    fn create_render_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> weaver_renderer::pipeline::RenderPipelineLayout
    where
        Self: Sized,
    {
        let bind_group_layout = bind_group_layout_cache.get_or_create::<Self>(device);

        RenderPipelineLayout::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("GizmoPipelineLayout"),
                bind_group_layouts: &[
                    &bind_group_layout,
                    &GpuCamera::create_bind_group_layout(device),
                ],
                push_constant_ranges: &[],
            }),
        )
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        cached_layout: &wgpu::PipelineLayout,
    ) -> RenderPipeline {
        let shader =
            Shader::new(Path::new("assets/shaders/gizmos.wgsl")).create_shader_module(device);
        RenderPipeline::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("GizmoRenderNode"),
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
                        format: texture_format::HDR_FORMAT,
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
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            }),
        )
    }
}

impl ViewNode for GizmoRenderNode {
    type Param = (
        Res<Gizmos>,
        Res<RenderCubeGizmo>,
        Res<RenderPipelineCache>,
        Res<HdrRenderTarget>,
    );
    type ViewQueryFetch = (&'static ViewTarget, &'static BindGroup<GpuCamera>);
    type ViewQueryFilter = ();

    fn prepare(&mut self, render_world: &WorldLock) -> Result<()> {
        let Some(gizmos) = render_world.get_resource::<Gizmos>() else {
            return Ok(());
        };

        self.transform_buffer.clear();
        self.color_buffer.clear();

        for gizmo in gizmos.gizmos.read().iter() {
            self.transform_buffer.push(gizmo.transform.matrix());
            self.color_buffer.push(gizmo.color);
        }

        let device = render_world.get_resource::<WgpuDevice>().unwrap();
        let queue = render_world.get_resource::<WgpuQueue>().unwrap();

        self.transform_buffer.enqueue_update(&device, &queue);
        self.color_buffer.enqueue_update(&device, &queue);

        if self.bind_group.is_none() {
            let mut layout_cache = render_world
                .get_resource_mut::<BindGroupLayoutCache>()
                .unwrap();
            let bind_group = BindGroup::new(&render_world.read(), &device, self, &mut layout_cache);
            self.bind_group = Some(bind_group);
        }

        let mut pipeline_cache = render_world
            .get_resource_mut::<RenderPipelineCache>()
            .unwrap();
        let mut bind_group_layout_cache = render_world
            .get_resource_mut::<BindGroupLayoutCache>()
            .unwrap();
        pipeline_cache.get_or_create_pipeline::<Self>(&device, &mut bind_group_layout_cache);

        Ok(())
    }

    fn run(
        &self,
        _render_world: &WorldLock,
        _graph_ctx: &mut weaver_renderer::graph::RenderGraphCtx,
        render_ctx: &mut weaver_renderer::graph::RenderCtx,
        (gizmos, cube_resource, pipeline_cache, hdr_target): &SystemParamItem<Self::Param>,
        (view_target, camera_bind_group): &QueryFetchItem<Self::ViewQueryFetch>,
    ) -> Result<()> {
        let pipeline = pipeline_cache.get_pipeline::<Self>().unwrap();

        let gizmo_bind_group = self.bind_group.as_ref().unwrap().bind_group();

        {
            let mut render_pass =
                render_ctx
                    .command_encoder()
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("GizmoRenderPass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: hdr_target.color_target(),
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
            render_pass.set_bind_group(0, gizmo_bind_group, &[]);
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

        Ok(())
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
        render_app.add_plugin(RenderPipelinePlugin::<GizmoRenderNode>::default())?;
        render_app.add_plugin(RenderResourcePlugin::<RenderCubeGizmo>::default())?;

        Ok(())
    }

    fn finish(&self, app: &mut App) -> Result<()> {
        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();

        render_app.add_render_main_graph_node::<ViewNodeRunner<GizmoRenderNode>>(GizmoNodeLabel);
        render_app.add_render_main_graph_edge(PbrNodeLabel, GizmoNodeLabel);
        render_app.add_render_main_graph_edge(GizmoNodeLabel, HdrNodeLabel);

        Ok(())
    }
}

fn clear_gizmos(gizmos: Res<Gizmos>) -> Result<()> {
    gizmos.clear();
    Ok(())
}
