//! GPU meshes and the mesh plugin.
//!
//! [`GpuMesh`] is the render asset for a [`Mesh`]. [`MeshPlugin`] also extracts `Handle<Mesh>`
//! components onto render entities and sets up instanced drawing (see [`instances`]); simple
//! generated meshes live in [`primitive`].

use weaver_app::{App, plugin::Plugin};
use weaver_core::{geometry::Aabb, mesh::Mesh};
use weaver_util::prelude::*;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use instances::{ViewMeshInstances, batch_opaque_meshes};
use weaver_ecs::system::IntoSystemConfig;

use crate::{
    RenderStage,
    asset::{ExtractHandlePlugin, PrepareAssetError, RenderAsset, RenderAssetPlugin},
    bind_group::ComponentBindGroupPlugin,
    phase::{Opaque3d, sort_phase},
};

pub mod instances;
pub mod primitive;

/// A mesh uploaded to the GPU.
///
/// The index buffer holds `u32` indices, so draw it with `IndexFormat::Uint32`.
#[derive(Debug)]
pub struct GpuMesh {
    /// Local-space bounds, for frustum culling.
    pub aabb: Aabb,
    /// Vertices laid out as [`vertex_buffer_layout`] describes.
    pub vertex_buffer: wgpu::Buffer,
    /// `u32` indices.
    pub index_buffer: wgpu::Buffer,
    /// Number of indices to draw.
    pub num_indices: u32,
}

impl RenderAsset for GpuMesh {
    type Source = Mesh;
    type Param = ();

    fn prepare(
        mesh: Mesh,
        _: &mut (),
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<Self, PrepareAssetError<Mesh>> {
        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Self {
            aabb: mesh.aabb,
            vertex_buffer,
            index_buffer,
            num_indices: mesh.indices.len() as u32,
        })
    }
}

/// Registers [`GpuMesh`], mesh handle extraction, and opaque mesh batching.
pub struct MeshPlugin;

impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_plugin(RenderAssetPlugin::<GpuMesh>::default())?;
        app.add_plugin(ExtractHandlePlugin::<Mesh>::default())?;
        app.add_plugin(ComponentBindGroupPlugin::<ViewMeshInstances>::default())?;
        app.add_system(
            batch_opaque_meshes.after(sort_phase::<Opaque3d>),
            RenderStage::Sort,
        );
        Ok(())
    }
}

/// The vertex buffer layout of [`weaver_core::mesh::Vertex`]: position, normal and tangent
/// (`vec3<f32>`, locations 0-2) and texture coordinates (`vec2<f32>`, location 3).
pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x3,
        3 => Float32x2,
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<weaver_core::mesh::Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    }
}
