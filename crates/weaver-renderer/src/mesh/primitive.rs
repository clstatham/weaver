use super::{Mesh, Vertex};

pub trait Primitive {
    fn generate_mesh(&self) -> Mesh;
}

#[rustfmt::skip]
const CUBE_VERTICES: &[glam::Vec3] = &[
    // front
    glam::Vec3::new(-1.0, -1.0, 1.0),
    glam::Vec3::new(1.0, -1.0, 1.0),
    glam::Vec3::new(1.0, 1.0, 1.0),
    glam::Vec3::new(-1.0, 1.0, 1.0),
    // back
    glam::Vec3::new(-1.0, -1.0, -1.0),
    glam::Vec3::new(1.0, -1.0, -1.0),
    glam::Vec3::new(1.0, 1.0, -1.0),
    glam::Vec3::new(-1.0, 1.0, -1.0),
    // top
    glam::Vec3::new(-1.0, 1.0, 1.0),
    glam::Vec3::new(1.0, 1.0, 1.0),
    glam::Vec3::new(1.0, 1.0, -1.0),
    glam::Vec3::new(-1.0, 1.0, -1.0),
    // bottom
    glam::Vec3::new(-1.0, -1.0, 1.0),
    glam::Vec3::new(1.0, -1.0, 1.0),
    glam::Vec3::new(1.0, -1.0, -1.0),
    glam::Vec3::new(-1.0, -1.0, -1.0),
    // left
    glam::Vec3::new(-1.0, -1.0, 1.0),
    glam::Vec3::new(-1.0, 1.0, 1.0),
    glam::Vec3::new(-1.0, 1.0, -1.0),
    glam::Vec3::new(-1.0, -1.0, -1.0),
    // right
    glam::Vec3::new(1.0, -1.0, 1.0),
    glam::Vec3::new(1.0, 1.0, 1.0),
    glam::Vec3::new(1.0, 1.0, -1.0),
    glam::Vec3::new(1.0, -1.0, -1.0),
];

#[rustfmt::skip]
const CUBE_NORMALS: &[glam::Vec3] = &[
    // front
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    // back
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    // top
    glam::Vec3::new(0.0, 1.0, 0.0),
    glam::Vec3::new(0.0, 1.0, 0.0),
    glam::Vec3::new(0.0, 1.0, 0.0),
    glam::Vec3::new(0.0, 1.0, 0.0),
    // bottom
    glam::Vec3::new(0.0, -1.0, 0.0),
    glam::Vec3::new(0.0, -1.0, 0.0),
    glam::Vec3::new(0.0, -1.0, 0.0),
    glam::Vec3::new(0.0, -1.0, 0.0),
    // left
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    // right
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
];

#[rustfmt::skip]
const CUBE_TANGENTS: &[glam::Vec3] = &[
    // front
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
    glam::Vec3::new(1.0, 0.0, 0.0),
    // back
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    glam::Vec3::new(-1.0, 0.0, 0.0),
    // top
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    // bottom
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    // left
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    glam::Vec3::new(0.0, 0.0, 1.0),
    // right
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
    glam::Vec3::new(0.0, 0.0, -1.0),
];

#[rustfmt::skip]
const CUBE_INDICES: &[u32] = &[
    // front
    0, 1, 2, 2, 3, 0,
    // back
    4, 5, 6, 6, 7, 4,
    // top
    8, 9, 10, 10, 11, 8,
    // bottom
    12, 13, 14, 14, 15, 12,
    // left
    16, 17, 18, 18, 19, 16,
    // right
    20, 21, 22, 22, 23, 20,
];

#[rustfmt::skip]
const CUBE_TEX_COORDS: &[glam::Vec2] = &[
    // front
    glam::Vec2::new(0.0, 0.0),
    glam::Vec2::new(1.0, 0.0),
    glam::Vec2::new(1.0, 1.0),
    glam::Vec2::new(0.0, 1.0),
    // back
    glam::Vec2::new(0.0, 0.0),
    glam::Vec2::new(1.0, 0.0),
    glam::Vec2::new(1.0, 1.0),
    glam::Vec2::new(0.0, 1.0),
    // top
    glam::Vec2::new(0.0, 0.0),
    glam::Vec2::new(1.0, 0.0),
    glam::Vec2::new(1.0, 1.0),
    glam::Vec2::new(0.0, 1.0),
    // bottom
    glam::Vec2::new(0.0, 0.0),
    glam::Vec2::new(1.0, 0.0),
    glam::Vec2::new(1.0, 1.0),
    glam::Vec2::new(0.0, 1.0),
    // left
    glam::Vec2::new(0.0, 0.0),
    glam::Vec2::new(1.0, 0.0),
    glam::Vec2::new(1.0, 1.0),
    glam::Vec2::new(0.0, 1.0),
    // right
    glam::Vec2::new(0.0, 0.0),
    glam::Vec2::new(1.0, 0.0),
    glam::Vec2::new(1.0, 1.0),
    glam::Vec2::new(0.0, 1.0),
];

pub struct CubePrimitive {
    pub side_length: f32,
}

impl CubePrimitive {
    pub fn new(side_length: f32) -> Self {
        Self { side_length }
    }
}

impl Primitive for CubePrimitive {
    fn generate_mesh(&self) -> Mesh {
        let mut vertices = Vec::with_capacity(CUBE_VERTICES.len());
        for i in 0..CUBE_VERTICES.len() {
            let position = CUBE_VERTICES[i] * self.side_length * 0.5;
            let normal = CUBE_NORMALS[i];
            let tangent = CUBE_TANGENTS[i];
            let tex_coords = CUBE_TEX_COORDS[i];

            vertices.push(Vertex {
                position,
                normal,
                tangent,
                tex_coords,
            });
        }

        let indices = CUBE_INDICES.to_vec();

        Mesh::new(vertices, indices)
    }
}
