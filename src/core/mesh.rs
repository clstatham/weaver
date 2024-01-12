use std::{path::Path, sync::Arc};

use weaver_proc_macro::Component;
use wgpu::util::DeviceExt;

use crate::{app::asset_server::AssetId, core::aabb::Aabb};

pub const MAX_MESHES: usize = 4096 * 8;

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
    pub tangent: glam::Vec3,
    pub uv: glam::Vec2,
}

impl Vertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<glam::Vec3>() as u64,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 2) as u64,
                    format: wgpu::VertexFormat::Float32x3,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<glam::Vec3>() * 3) as u64,
                    format: wgpu::VertexFormat::Float32x2,
                    shader_location: 3,
                },
            ],
        }
    }
}

fn calculate_tangents(vertices: &mut [Vertex], indices: &[u32]) {
    for vertex in vertices.iter_mut() {
        vertex.tangent = glam::Vec3::ZERO;
    }

    let mut num_triangles = vec![0; vertices.len()];
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

        vertices[i0].tangent += tangent;
        vertices[i1].tangent += tangent;
        vertices[i2].tangent += tangent;

        num_triangles[i0] += 1;
        num_triangles[i1] += 1;
        num_triangles[i2] += 1;
    }

    for (vertex, num_triangles) in vertices.iter_mut().zip(num_triangles) {
        vertex.tangent /= num_triangles as f32;

        // gram-schmidt orthogonalize
        let tangent = vertex.tangent - vertex.normal * vertex.normal.dot(vertex.tangent);
        vertex.tangent = tangent.normalize();

        // check for orthogonality
        let ndt = vertex.normal.dot(vertex.tangent);
        assert!(
            ndt < 0.001,
            "normal and tangent are not orthogonal: N . T = {:?}",
            ndt
        );

        // sanity check with the binormal
        let binormal = vertex.normal.cross(vertex.tangent);
        let ndb = vertex.normal.dot(binormal);
        assert!(
            ndb < 0.001,
            "normal and binormal are not orthogonal: N . B = {:?}",
            ndb
        );
        let bdt = binormal.dot(vertex.tangent);
        assert!(
            bdt < 0.001,
            "binormal and tangent are not orthogonal: B . T = {:?}",
            bdt
        );

        // calculate handedness
        let tangent = if vertex.normal.cross(vertex.tangent).dot(vertex.tangent) < 0.0 {
            -vertex.tangent
        } else {
            vertex.tangent
        };
        vertex.tangent = tangent;
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
                        normal: glam::Vec3::from(normal).normalize(),
                        uv: glam::Vec2::from(uv),
                        tangent: glam::Vec3::ZERO,
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

    pub(crate) fn load_obj(
        path: impl AsRef<Path>,
        device: &wgpu::Device,
        asset_id: AssetId,
    ) -> anyhow::Result<Self> {
        let path = path;
        let (models, _) = tobj::load_obj(
            path.as_ref(),
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
        )?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for model in models {
            let mesh = model.mesh;

            for i in 0..mesh.positions.len() / 3 {
                let position = [
                    mesh.positions[i * 3],
                    mesh.positions[i * 3 + 1],
                    mesh.positions[i * 3 + 2],
                ];
                let normal = [
                    mesh.normals[i * 3],
                    mesh.normals[i * 3 + 1],
                    mesh.normals[i * 3 + 2],
                ];
                let uv = [mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1]];

                vertices.push(Vertex {
                    position: glam::Vec3::from(position),
                    normal: glam::Vec3::from(normal).normalize(),
                    uv: glam::Vec2::from(uv),
                    tangent: glam::Vec3::ZERO,
                });
            }

            for index in mesh.indices {
                indices.push(index);
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
