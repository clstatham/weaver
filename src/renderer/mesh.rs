use super::color::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub color: Color,
    pub normal: Option<glam::Vec3>,
    // pub uv: glam::Vec2,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    pub fn calculate_normals(&mut self) {
        for vertex in &mut self.vertices {
            vertex.normal = Some(glam::Vec3::ZERO);
        }

        // Calculate normals for each face with counter-clockwise winding.
        for i in (0..self.indices.len()).step_by(3) {
            let i0 = self.indices[i] as usize;
            let i1 = self.indices[i + 1] as usize;
            let i2 = self.indices[i + 2] as usize;

            let v0 = self.vertices[i0].position;
            let v1 = self.vertices[i1].position;
            let v2 = self.vertices[i2].position;

            let normal = (v1 - v0).cross(v2 - v0).normalize();

            self.vertices[i0].normal = Some(self.vertices[i0].normal.unwrap() + normal);
            self.vertices[i1].normal = Some(self.vertices[i1].normal.unwrap() + normal);
            self.vertices[i2].normal = Some(self.vertices[i2].normal.unwrap() + normal);
        }

        for vertex in &mut self.vertices {
            vertex.normal = Some(vertex.normal.unwrap().normalize());
        }
    }
}
