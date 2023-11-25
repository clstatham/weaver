use super::color::Color;

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub color: Color,
    // pub uv: glam::Vec2,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub normals: Vec<glam::Vec3>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let mut mesh = Self {
            vertices,
            indices,
            normals: vec![],
        };

        mesh.calculate_normals();

        mesh
    }

    pub fn calculate_normals(&mut self) {
        self.normals = vec![glam::Vec3::ZERO; self.vertices.len()];

        for i in (0..self.indices.len()).step_by(3) {
            let i0 = self.indices[i] as usize;
            let i1 = self.indices[i + 1] as usize;
            let i2 = self.indices[i + 2] as usize;

            let v0 = self.vertices[i0].position;
            let v1 = self.vertices[i1].position;
            let v2 = self.vertices[i2].position;

            let normal = (v1 - v0).cross(v2 - v0).normalize();

            self.normals[i0] += normal;
            self.normals[i1] += normal;
            self.normals[i2] += normal;
        }

        for normal in &mut self.normals {
            *normal = normal.normalize();
        }
    }
}
