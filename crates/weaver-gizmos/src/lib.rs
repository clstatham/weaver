use std::{path::Path, sync::Arc};

use weaver_app::{plugin::Plugin, App, AppStage};
use weaver_core::{color::Color, prelude::Mat4, transform::Transform};
use weaver_ecs::{
    component::Res,
    prelude::ResMut,
    query::Query,
    system::IntoSystemConfig,
    world::{ConstructFromWorld, World},
};
// use weaver_pbr::render::PbrRenderable;
use weaver_renderer::{
    bind_group::{BindGroup, CreateBindGroup},
    buffer::{GpuBuffer, GpuBufferVec},
    camera::{CameraBindGroup, ViewTarget},
    hdr::{render_hdr, HdrRenderTarget},
    mesh::primitive::{CubePrimitive, Primitive},
    pipeline::{RenderPipeline, RenderPipelineLayout},
    prelude::*,
    resources::ActiveCommandEncoder,
    shader::Shader,
    texture::{texture_format, GpuTexture},
    RenderApp, RenderLabel, RenderStage, WgpuDevice, WgpuQueue,
};
use weaver_util::prelude::*;

use wgpu::util::DeviceExt;

pub mod prelude {
    pub use super::{Gizmo, GizmoMode, GizmoPlugin, Gizmos};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gizmo {
    Cube,
}

pub struct SolidCubeGizmo {
    pub vertex_buffer: GpuBuffer,
    pub index_buffer: GpuBuffer,
    pub num_indices: usize,
}

impl ConstructFromWorld for SolidCubeGizmo {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();

        let cube = CubePrimitive::new(1.0, false);
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

        Self {
            vertex_buffer: GpuBuffer::from(vertex_buffer),
            index_buffer: GpuBuffer::from(index_buffer),
            num_indices: mesh.indices.len(),
        }
    }
}

pub struct WireCubeGizmo {
    pub vertex_buffer: GpuBuffer,
    pub index_buffer: GpuBuffer,
    pub num_indices: usize,
}

impl ConstructFromWorld for WireCubeGizmo {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();

        let cube = CubePrimitive::new(1.0, true);
        let mesh = cube.generate_mesh();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("WireCubeVertexBuffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("WireCubeIndexBuffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer: GpuBuffer::from(vertex_buffer),
            index_buffer: GpuBuffer::from(index_buffer),
            num_indices: mesh.indices.len(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GizmoMode {
    Solid,
    Wireframe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GizmoKey {
    pub mode: GizmoMode,
    pub depth_test: bool,
}

pub struct GizmoInstance {
    pub gizmo: Gizmo,
    pub color: Color,
    pub transform: Transform,
}

impl GizmoInstance {
    pub fn new(gizmo: Gizmo, color: Color, transform: Transform) -> Self {
        Self {
            gizmo,
            color,
            transform,
        }
    }
}

#[derive(Clone)]
pub struct Gizmos {
    pub(crate) gizmos: SharedLock<FxHashMap<GizmoKey, Vec<GizmoInstance>>>,
}

impl Default for Gizmos {
    fn default() -> Self {
        Self {
            gizmos: SharedLock::new(FxHashMap::default()),
        }
    }
}

impl Gizmos {
    pub fn add_gizmo(&self, key: GizmoKey, gizmo: GizmoInstance) {
        self.gizmos.write().entry(key).or_default().push(gizmo);
    }

    pub fn clear(&self) {
        self.gizmos.write().clear();
    }

    pub fn solid_cube(&self, transform: Transform, color: Color) {
        self.add_gizmo(
            GizmoKey {
                mode: GizmoMode::Solid,
                depth_test: true,
            },
            GizmoInstance::new(Gizmo::Cube, color, transform),
        );
    }

    pub fn wire_cube(&self, transform: Transform, color: Color) {
        self.add_gizmo(
            GizmoKey {
                mode: GizmoMode::Wireframe,
                depth_test: true,
            },
            GizmoInstance::new(Gizmo::Cube, color, transform),
        );
    }

    pub fn solid_cube_no_depth(&self, transform: Transform, color: Color) {
        self.add_gizmo(
            GizmoKey {
                mode: GizmoMode::Solid,
                depth_test: false,
            },
            GizmoInstance::new(Gizmo::Cube, color, transform),
        );
    }

    pub fn wire_cube_no_depth(&self, transform: Transform, color: Color) {
        self.add_gizmo(
            GizmoKey {
                mode: GizmoMode::Wireframe,
                depth_test: false,
            },
            GizmoInstance::new(Gizmo::Cube, color, transform),
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GizmoSubGraph;
impl RenderLabel for GizmoSubGraph {}

#[derive(Debug, Clone, Copy)]
pub struct GizmoNodeLabel;
impl RenderLabel for GizmoNodeLabel {}

#[derive(Default)]
pub struct GizmoRenderable {
    transform_buffer: FxHashMap<GizmoKey, GpuBufferVec<Mat4>>,
    color_buffer: FxHashMap<GizmoKey, GpuBufferVec<Color>>,
    bind_group: FxHashMap<GizmoKey, Arc<wgpu::BindGroup>>,
    pipelines: FxHashMap<GizmoKey, RenderPipeline>,
    gizmo_depth_texture: Option<GpuTexture>,
}

impl GizmoRenderable {
    pub fn get_or_create_buffers(
        &mut self,
        key: GizmoKey,
    ) -> (&mut GpuBufferVec<Mat4>, &mut GpuBufferVec<Color>) {
        let transform_buffer = self.transform_buffer.entry(key).or_insert_with(|| {
            GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
        });
        let color_buffer = self.color_buffer.entry(key).or_insert_with(|| {
            GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
        });

        (transform_buffer, color_buffer)
    }

    pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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

    pub fn get_or_create_bind_group(
        &mut self,
        device: &wgpu::Device,
        key: GizmoKey,
    ) -> Arc<wgpu::BindGroup> {
        if let Some(bind_group) = self.bind_group.get(&key) {
            return bind_group.clone();
        }
        let layout = Self::create_bind_group_layout(device);
        let (transform_buffer, color_buffer) = self.get_or_create_buffers(key);

        let bind_group = Arc::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GizmoBindGroup"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: transform_buffer.binding().unwrap(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: color_buffer.binding().unwrap(),
                },
            ],
        }));

        self.bind_group.insert(key, bind_group.clone());
        bind_group
    }

    pub fn create_render_pipeline_layout(
        device: &wgpu::Device,
    ) -> weaver_renderer::pipeline::RenderPipelineLayout
    where
        Self: Sized,
    {
        let bind_group_layout = Self::create_bind_group_layout(device);

        RenderPipelineLayout::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("GizmoPipelineLayout"),
                bind_group_layouts: &[
                    &bind_group_layout,
                    &CameraBindGroup::create_bind_group_layout(device),
                ],
                push_constant_ranges: &[],
            }),
        )
    }

    pub fn get_or_create_pipeline(
        &mut self,
        device: &wgpu::Device,
        key: GizmoKey,
    ) -> RenderPipeline {
        if let Some(pipeline) = self.pipelines.get(&key) {
            return pipeline.clone();
        }

        let layout = Self::create_render_pipeline_layout(device);

        const VERTEX_BUFFER_LAYOUT: wgpu::VertexBufferLayout = wgpu::VertexBufferLayout {
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
        };

        let shader =
            Shader::new(Path::new("assets/shaders/gizmos.wgsl")).create_shader_module(device);

        let primitive_topology = match key.mode {
            GizmoMode::Solid => wgpu::PrimitiveTopology::TriangleList,
            GizmoMode::Wireframe => wgpu::PrimitiveTopology::LineList,
        };

        let pipeline = RenderPipeline::new(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("GizmoRenderPipeline"),
                layout: Some(&layout),
                cache: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[VERTEX_BUFFER_LAYOUT],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: texture_format::HDR_FORMAT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: primitive_topology,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            },
        ));

        self.pipelines.insert(key, pipeline.clone());
        pipeline
    }
}

pub async fn prepare_gizmos(
    gizmos: Res<Gizmos>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
    hdr_target: Res<HdrRenderTarget>,
    mut gizmo_renderable: ResMut<GizmoRenderable>,
) {
    if gizmo_renderable.gizmo_depth_texture.is_none() {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GizmoDepthTexture"),
            size: hdr_target.texture.texture.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        gizmo_renderable.gizmo_depth_texture = Some(GpuTexture {
            texture: Arc::new(depth_texture),
            view: Arc::new(depth_view),
        });
    }

    for (key, instances) in gizmos.gizmos.read().iter() {
        let (transform_buffer, color_buffer) = gizmo_renderable.get_or_create_buffers(*key);

        transform_buffer.clear();
        color_buffer.clear();

        for gizmo in instances.iter() {
            transform_buffer.push(gizmo.transform.matrix());
            color_buffer.push(gizmo.color);
        }

        transform_buffer.enqueue_update(&device, &queue);
        color_buffer.enqueue_update(&device, &queue);

        gizmo_renderable.get_or_create_bind_group(&device, *key);
        gizmo_renderable.get_or_create_pipeline(&device, *key);
    }
}

pub async fn render_gizmos(
    gizmos: Res<Gizmos>,
    solid_cube: Res<SolidCubeGizmo>,
    wire_cube: Res<WireCubeGizmo>,
    hdr_target: Res<HdrRenderTarget>,
    mut view_query: Query<(&ViewTarget, &BindGroup<CameraBindGroup>)>,
    mut command_encoder: ResMut<ActiveCommandEncoder>,
    gizmo_renderable: Res<GizmoRenderable>,
) {
    let depth_texture = gizmo_renderable.gizmo_depth_texture.as_ref().unwrap();
    {
        let _clear_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GizmoDepthClearPass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_texture.view,
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

    for (view_target, camera_bind_group) in view_query.iter() {
        for key in gizmos.gizmos.read().keys() {
            let gizmo_bind_group = gizmo_renderable.bind_group.get(key).unwrap();
            let pipeline = gizmo_renderable.pipelines.get(key).unwrap();

            let depth_stencil_attachment = if key.depth_test {
                Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view_target.depth_target,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                })
            } else {
                // render to our own depth texture
                Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                })
            };

            let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GizmoRenderPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: hdr_target.color_target(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, gizmo_bind_group, &[]);
            render_pass.set_bind_group(1, &camera_bind_group, &[]);

            let mut num_cubes = 0;
            for instance in gizmos.gizmos.read().get(key).unwrap().iter() {
                match instance.gizmo {
                    Gizmo::Cube => {
                        num_cubes += 1;
                    }
                }
            }

            match key.mode {
                GizmoMode::Solid => {
                    render_pass.set_vertex_buffer(0, solid_cube.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        solid_cube.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.draw_indexed(
                        0..solid_cube.num_indices as u32,
                        0,
                        0..num_cubes as u32,
                    );
                }
                GizmoMode::Wireframe => {
                    render_pass.set_vertex_buffer(0, wire_cube.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        wire_cube.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.draw_indexed(
                        0..wire_cube.num_indices as u32,
                        0,
                        0..num_cubes as u32,
                    );
                }
            }
        }
    }
}

pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        let gizmos = Gizmos::default();
        app.insert_resource(gizmos.clone());
        app.add_system(clear_gizmos, AppStage::PrepareFrame);

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app
            .add_plugin(GizmoRenderAppPlugin { gizmos })
            .unwrap();

        Ok(())
    }
}

pub struct GizmoRenderAppPlugin {
    gizmos: Gizmos,
}

impl Plugin for GizmoRenderAppPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.insert_resource(self.gizmos.clone());

        render_app.add_system(prepare_gizmos, RenderStage::PreRender);

        render_app.add_system(render_gizmos.before(render_hdr), RenderStage::Render);
        Ok(())
    }

    fn ready(&self, app: &App) -> bool {
        app.has_resource::<WgpuDevice>()
    }

    fn finish(&self, render_app: &mut App) -> Result<()> {
        render_app.init_resource::<SolidCubeGizmo>();
        render_app.init_resource::<WireCubeGizmo>();

        Ok(())
    }
}

async fn clear_gizmos(gizmos: Res<Gizmos>) {
    gizmos.clear();
}
