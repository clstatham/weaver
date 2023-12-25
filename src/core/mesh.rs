use crate::ecs::component::Component;

use super::{color::Color, texture::Texture, Vertex};

#[derive(Clone)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub texture: Option<Texture>,
}
impl Component for Mesh {}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>, texture: Option<Texture>) -> Self {
        Self {
            vertices,
            indices,
            texture,
        }
    }

    pub fn from_vertices(vertices: Vec<Vertex>) -> Self {
        let indices = (0..vertices.len() as u32).collect();

        Self {
            vertices,
            indices,
            texture: None,
        }
    }

    pub fn recalculate_normals(&mut self) {
        for vertex in self.vertices.iter_mut() {
            vertex.normal = glam::Vec3A::ZERO;
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
        let mut uvs = Vec::new();
        let mut vertex_colors = Vec::new();

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

            positions.push(glam::Vec3A::new(x, y, z));
        }

        for i in 0..mesh.normals.len() / 3 {
            let x = mesh.normals[i * 3];
            let y = mesh.normals[i * 3 + 1];
            let z = mesh.normals[i * 3 + 2];

            normals.push(glam::Vec3A::new(x, y, z));
        }

        for i in 0..mesh.texcoords.len() / 2 {
            let u = mesh.texcoords[i * 2];
            let v = mesh.texcoords[i * 2 + 1];

            uvs.push(glam::Vec2::new(u, v));
        }

        for i in 0..mesh.vertex_color.len() / 3 {
            let r = mesh.vertex_color[i * 3];
            let g = mesh.vertex_color[i * 3 + 1];
            let b = mesh.vertex_color[i * 3 + 2];

            vertex_colors.push(Color::new(r, g, b));
        }

        for (position, normal, color, uv) in
            itertools::multizip((positions, normals, vertex_colors, uvs))
        {
            vertices.push(Vertex {
                position,
                color,
                normal,
                uv,
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

        Ok(Mesh::new(vertices, indices, None))
    }

    pub fn load_gltf(path: impl AsRef<std::path::Path>) -> anyhow::Result<Mesh> {
        let (document, buffers, images) = gltf::import(path.as_ref())?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let positions = reader.read_positions().unwrap();
                let normals = reader.read_normals().unwrap();
                let uvs = reader.read_tex_coords(0).unwrap().into_f32();
                // let vertex_colors = reader.read_colors(0).unwrap().into_rgb_f32();

                for (position, normal, uv) in itertools::multizip((positions, normals, uvs)) {
                    vertices.push(Vertex {
                        position: position.into(),
                        // color: Color::new(color[0], color[1], color[2]),
                        color: Color::WHITE,
                        normal: normal.into(),
                        uv: uv.into(),
                    });
                }

                let index_reader = reader.read_indices().unwrap().into_u32();
                for index in index_reader {
                    indices.push(index);
                }
            }
        }

        // load texture
        let texture = if let Some(image) = images.into_iter().next() {
            log::info!("Loading texture");
            let texture = Texture::from_data_r8g8b8(
                image.width as usize,
                image.height as usize,
                &image.pixels,
            );
            Some(texture)
        } else {
            None
        };

        Ok(Mesh::new(vertices, indices, texture))
    }
}
