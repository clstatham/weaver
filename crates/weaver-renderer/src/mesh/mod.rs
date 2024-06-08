use crate::{
    extract::{ExtractRenderComponentPlugin, RenderComponent},
    Renderer,
};
use weaver_app::{plugin::Plugin, App};
use weaver_asset::{Assets, Handle};
use weaver_core::mesh::Mesh;
use weaver_ecs::prelude::*;
use weaver_util::prelude::*;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub mod primitive;

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl RenderComponent for GpuMesh {
    fn query() -> Query {
        Query::new().read::<Handle<Mesh>>()
    }

    fn extract_render_component(entity: Entity, world: &World) -> Option<Self>
    where
        Self: Sized,
    {
        let renderer = world.get_resource::<Renderer>()?;
        let assets = world.get_resource::<Assets>()?;
        let mesh = world.get_component::<Handle<Mesh>>(entity)?;
        let mesh = assets.get(*mesh)?;

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
