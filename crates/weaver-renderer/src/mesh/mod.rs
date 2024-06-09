use crate::{
    asset::{ExtractRenderAssetPlugin, RenderAsset},
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

impl RenderAsset for GpuMesh {
    type BaseAsset = Mesh;

    fn extract_render_asset(base_asset: &Mesh, _world: &World, renderer: &Renderer) -> Option<Self>
    where
        Self: Sized,
    {
        let vertex_buffer = renderer.device().create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&base_asset.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = renderer.device().create_buffer_init(&BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&base_asset.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Some(Self {
            vertex_buffer,
            index_buffer,
            num_indices: base_asset.indices.len() as u32,
        })
    }
}

pub struct MeshPlugin;

impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(ExtractRenderAssetPlugin::<GpuMesh>::default())?;
        Ok(())
    }
}
