use crate::{
    extract::{RenderComponent, RenderComponentPlugin},
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
    fn extract_query() -> Query {
        Query::new().read::<Handle<Mesh>>()
    }

    fn extract_render_component(entity: Entity, world: &World, renderer: &Renderer) -> Option<Self>
    where
        Self: Sized,
    {
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

    fn update_render_component(
        &mut self,
        _entity: Entity,
        _world: &World,
        _renderer: &Renderer,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct MeshPlugin;

impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(RenderComponentPlugin::<GpuMesh>::default())?;
        Ok(())
    }
}
