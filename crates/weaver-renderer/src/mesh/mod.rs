use crate::{
    extract::{ExtractRenderComponentPlugin, RenderComponent},
    Renderer,
};
use weaver_app::{plugin::Plugin, App};
use weaver_ecs::prelude::*;
use weaver_util::prelude::*;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub mod primitive;

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
    pub tangent: glam::Vec3,
    pub tex_coords: glam::Vec2,
}

impl Vertex {
    pub fn buffer_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    offset: 3 * 4,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    offset: 6 * 4,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    offset: 9 * 4,
                    format: wgpu::VertexFormat::Float32x2,
                    shader_location: 3,
                },
            ],
        }
    }
}

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }
}

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl RenderComponent for GpuMesh {
    fn query() -> Query {
        Query::new().read::<Mesh>()
    }

    fn extract_render_component(entity: Entity, world: &World) -> Option<Self>
    where
        Self: Sized,
    {
        let renderer = world.get_resource::<Renderer>()?;
        let mesh = world.get_component::<Mesh>(entity)?;

        let vertex_buffer = renderer.device().create_buffer_init(&BufferInitDescriptor {
            label: Some("Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = renderer.device().create_buffer_init(&BufferInitDescriptor {
            label: Some("Mesh Index Buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Some(Self {
            vertex_buffer,
            index_buffer,
            num_indices: mesh.indices.len() as u32,
        })
    }
}

pub struct MeshPlugin;

impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(ExtractRenderComponentPlugin::<GpuMesh>::default())?;
        Ok(())
    }
}
