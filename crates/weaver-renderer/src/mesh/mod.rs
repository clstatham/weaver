use crate::asset::{ExtractRenderAssetPlugin, RenderAsset};
use weaver_app::{plugin::Plugin, App};
use weaver_asset::{prelude::Asset, Assets};
use weaver_core::mesh::Mesh;
use weaver_util::prelude::*;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub mod primitive;

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl Asset for GpuMesh {
    fn load(_assets: &mut Assets, _path: &std::path::Path) -> Result<Self> {
        bail!("GpuMesh cannot be loaded from a file")
    }
}

impl RenderAsset for GpuMesh {
    type BaseAsset = Mesh;

    fn extract_render_asset(
        base_asset: &Mesh,
        _main_world_assets: &Assets,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&base_asset.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
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

    fn update_render_asset(
        &mut self,
        _base_asset: &Self::BaseAsset,
        _main_world_assets: &Assets,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}

pub struct MeshPlugin;

impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(ExtractRenderAssetPlugin::<GpuMesh>::default())?;
        app.register_type::<Mesh>();
        Ok(())
    }
}
