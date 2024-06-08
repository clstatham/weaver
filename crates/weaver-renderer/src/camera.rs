use std::fmt::Debug;

use wgpu::util::DeviceExt;

use weaver_app::plugin::Plugin;
use weaver_ecs::{prelude::Entity, query::Query, world::World};

use crate::{
    bind_group::{CreateBindGroup, CreateBindGroupPlugin},
    buffer::GpuBuffer,
    extract::{ExtractRenderComponentPlugin, RenderComponent},
    graph::RenderGraph,
    Renderer,
};

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub inv_view: glam::Mat4,
    pub inv_proj: glam::Mat4,
    pub camera_position: glam::Vec3,
    pub _padding: u32,
}

impl From<&Camera> for CameraUniform {
    fn from(camera: &Camera) -> Self {
        let view = camera.view_matrix;
        let proj = camera.projection_matrix;
        let inv_view = view.inverse();
        let inv_proj = proj.inverse();
        let camera_position = inv_view.col(3).truncate();

        Self {
            view,
            proj,
            inv_view,
            inv_proj,
            camera_position,
            _padding: 0,
        }
    }
}

pub struct Camera {
    pub active: bool,
    pub graph: RenderGraph,
    pub view_matrix: glam::Mat4,
    pub projection_matrix: glam::Mat4,
}

impl Debug for Camera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Camera")
            .field("view_matrix", &self.view_matrix)
            .field("projection_matrix", &self.projection_matrix)
            .finish()
    }
}

impl Camera {
    pub fn new(view_matrix: glam::Mat4, projection_matrix: glam::Mat4) -> Self {
        let graph = RenderGraph::new();

        Self {
            active: true,
            graph,
            view_matrix,
            projection_matrix,
        }
    }

    pub fn perspective_lookat(
        eye: glam::Vec3,
        center: glam::Vec3,
        up: glam::Vec3,
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self::new(
            glam::Mat4::look_at_rh(eye, center, up),
            glam::Mat4::perspective_rh(fov, aspect, near, far),
        )
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn activate(&mut self) {
        self.set_active(true);
    }

    pub fn deactivate(&mut self) {
        self.set_active(false);
    }

    pub fn render_graph(&self) -> &RenderGraph {
        &self.graph
    }

    pub fn render_graph_mut(&mut self) -> &mut RenderGraph {
        &mut self.graph
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(glam::Mat4::IDENTITY, glam::Mat4::IDENTITY)
    }
}

pub struct GpuCamera {
    pub uniform_buffer: GpuBuffer,
}

impl RenderComponent for GpuCamera {
    fn query() -> Query {
        Query::new().read::<Camera>()
    }

    fn extract_render_component(entity: Entity, world: &World) -> Option<Self>
    where
        Self: Sized,
    {
        let renderer = world.get_resource::<Renderer>()?;
        let camera = world.get_component::<Camera>(entity)?;

        let uniform_buffer =
            renderer
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera Uniform Buffer"),
                    contents: bytemuck::cast_slice(&[CameraUniform::from(&*camera)]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        Some(Self {
            uniform_buffer: GpuBuffer::new(uniform_buffer),
        })
    }
}

impl CreateBindGroup for GpuCamera {
    fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &Self::bind_group_layout(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
            label: Some("Camera Bind Group"),
        })
    }

    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut weaver_app::App) -> anyhow::Result<()> {
        app.add_plugin(ExtractRenderComponentPlugin::<GpuCamera>::default())?;
        app.add_plugin(CreateBindGroupPlugin::<GpuCamera>::default())?;

        Ok(())
    }
}
