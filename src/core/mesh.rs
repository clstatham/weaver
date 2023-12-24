use crate::ecs::component::Component;

use super::{color::Color, Vertex};

#[derive(Clone)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}
impl Component for Mesh {}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    pub fn from_vertices(vertices: Vec<Vertex>) -> Self {
        let indices = (0..vertices.len() as u32).collect();

        Self { vertices, indices }
    }

    pub fn recalculate_normals(&mut self) {
        for vertex in self.vertices.iter_mut() {
            vertex.normal = glam::Vec3::ZERO;
        }

        for i in (0..self.indices.len()).step_by(3) {
            let i0 = self.indices[i] as usize;
            let i1 = self.indices[i + 1] as usize;
            let i2 = self.indices[i + 2] as usize;

            let v0 = self.vertices[i0].position;
            let v1 = self.vertices[i1].position;
            let v2 = self.vertices[i2].position;

            let normal = (v1 - v0).cross(v2 - v0).normalize();

            self.vertices[i0].normal += normal;
            self.vertices[i1].normal += normal;
            self.vertices[i2].normal += normal;
        }

        for vertex in self.vertices.iter_mut() {
            vertex.normal = vertex.normal.normalize();
        }
    }

    pub fn load_obj(path: impl AsRef<std::path::Path>) -> anyhow::Result<Mesh> {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let (models, _materials) = tobj::load_obj(
            path.as_ref(),
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ignore_lines: true,
                ignore_points: true,
            },
        )?;

        let model = &models[0];
        let mesh = &model.mesh;

        for i in 0..mesh.positions.len() / 3 {
            let x = mesh.positions[i * 3];
            let y = mesh.positions[i * 3 + 1];
            let z = mesh.positions[i * 3 + 2];

            positions.push(glam::Vec3::new(x, y, z));
        }

        for i in 0..mesh.normals.len() / 3 {
            let x = mesh.normals[i * 3];
            let y = mesh.normals[i * 3 + 1];
            let z = mesh.normals[i * 3 + 2];

            normals.push(glam::Vec3::new(x, y, z));
        }

        for (position, normal) in positions.iter().zip(normals.iter()) {
            vertices.push(Vertex {
                position: *position,
                color: Color::new(1.0, 1.0, 1.0),
                normal: *normal,
            });
        }

        for i in 0..mesh.indices.len() / 3 {
            let i0 = mesh.indices[i * 3] as usize;
            let i1 = mesh.indices[i * 3 + 1] as usize;
            let i2 = mesh.indices[i * 3 + 2] as usize;

            indices.push(i0 as u32);
            indices.push(i1 as u32);
            indices.push(i2 as u32);
        }

        Ok(Mesh::new(vertices, indices))
    }

    pub fn load_gltf(path: impl AsRef<std::path::Path>) -> anyhow::Result<Mesh> {
        let (document, buffers, _images) = gltf::import(path.as_ref())?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let positions = reader.read_positions().unwrap();
                let normals = reader.read_normals().unwrap();

                for (position, normal) in positions.zip(normals) {
                    vertices.push(Vertex {
                        position: position.into(),
                        color: Color::new(1.0, 1.0, 1.0),
                        normal: normal.into(),
                    });
                }

                let index_reader = reader.read_indices().unwrap().into_u32();
                for index in index_reader {
                    indices.push(index);
                }
            }
        }

        Ok(Mesh::new(vertices, indices))
    }
}
