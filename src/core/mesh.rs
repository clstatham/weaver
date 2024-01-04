use std::{path::Path, sync::Arc};

use weaver_proc_macro::Component;
use wgpu::util::DeviceExt;

use crate::{app::asset_server::AssetId, core::aabb::Aabb};

pub const MAX_MESHES: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
    pub binormal: glam::Vec3,
    pub tangent: glam::Vec3,
    pub bitangent: glam::Vec3,
    pub uv: glam::Vec2,
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // normal
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<glam::Vec3>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // binormal
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 2) as u64,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // tangent
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 3) as u64,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // bitangent
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 4) as u64,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // uv
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 5) as u64,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

fn calculate_tangents(vertices: &mut [Vertex], indices: &[u32]) {
    for c in indices.chunks(3) {
        let i0 = c[0] as usize;
        let i1 = c[1] as usize;
        let i2 = c[2] as usize;

        let v0 = vertices[i0].position;
        let v1 = vertices[i1].position;
        let v2 = vertices[i2].position;

        let uv0 = vertices[i0].uv;
        let uv1 = vertices[i1].uv;
        let uv2 = vertices[i2].uv;

        let delta_pos1 = v1 - v0;
        let delta_pos2 = v2 - v0;

        let delta_uv1 = uv1 - uv0;
        let delta_uv2 = uv2 - uv0;

        let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
        let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
        let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * r;

        vertices[i0].tangent += tangent;
        vertices[i1].tangent += tangent;
        vertices[i2].tangent += tangent;

        vertices[i0].bitangent += bitangent;
        vertices[i1].bitangent += bitangent;
        vertices[i2].bitangent += bitangent;
    }

    for vertex in vertices.iter_mut() {
        vertex.tangent = vertex.tangent.normalize();
        vertex.bitangent = vertex.bitangent.normalize();
        vertex.binormal = vertex.normal.cross(vertex.tangent).normalize();
    }
}

struct MeshInner {
    pub asset_id: AssetId,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: usize,
    pub aabb: Aabb,
}

#[derive(Clone, Component)]
pub struct Mesh {
    inner: Arc<MeshInner>,
}

impl Mesh {
    pub(crate) fn load_gltf(
        path: impl AsRef<Path>,
        device: &wgpu::Device,
        asset_id: AssetId,
    ) -> anyhow::Result<Self> {
        let path = path;
        let (document, buffers, _images) = gltf::import(path.as_ref())?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let positions = reader.read_positions().unwrap();
                let normals = reader.read_normals().unwrap();
                let uvs = reader.read_tex_coords(0).unwrap().into_f32();

                for (position, normal, uv) in itertools::multizip((positions, normals, uvs)) {
                    vertices.push(Vertex {
                        position: glam::Vec3::from(position),
                        normal: glam::Vec3::from(normal),
                        uv: glam::Vec2::from(uv),
                        binormal: glam::Vec3::ZERO,
                        tangent: glam::Vec3::ZERO,
                        bitangent: glam::Vec3::ZERO,
                    });
                }

                let index_reader = reader.read_indices().unwrap().into_u32();
                for index in index_reader {
                    indices.push(index);
                }
            }
        }

        calculate_tangents(&mut vertices, &indices);

        let aabb = Aabb::from_points(&vertices.iter().map(|v| v.position).collect::<Vec<_>>());

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        log::info!("Loaded mesh: {}", path.as_ref().display());
        log::info!("  Vertices: {}", vertices.len());
        log::info!("    Size: {} bytes", vertex_buffer.size());
        log::info!("  Indices: {}", indices.len());
        log::info!("    Size: {} bytes", index_buffer.size());

        Ok(Self {
            inner: Arc::new(MeshInner {
                asset_id,
                vertex_buffer,
                index_buffer,
                num_indices: indices.len(),
                aabb,
            }),
        })
    }

    pub fn asset_id(&self) -> AssetId {
        self.inner.asset_id
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.inner.vertex_buffer
    }

    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.inner.index_buffer
    }

    pub fn num_indices(&self) -> usize {
        self.inner.num_indices
    }

    pub fn aabb(&self) -> Aabb {
        self.inner.aabb
    }
}
