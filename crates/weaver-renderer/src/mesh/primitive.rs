use weaver_core::mesh::{Mesh, Vertex};

pub trait Primitive {
    fn generate_mesh(&self) -> Mesh;
}

#[rustfmt::skip]
const CUBE_VERTICES: &[glam::Vec3] = &[
    // front
    glam::Vec3::new(-0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, 0.5),
    glam::Vec3::new(-0.5, 0.5, 0.5),
    // back
    glam::Vec3::new(-0.5, -0.5, -0.5),
    glam::Vec3::new(0.5, -0.5, -0.5),
    glam::Vec3::new(0.5, 0.5, -0.5),
    glam::Vec3::new(-0.5, 0.5, -0.5),
    // top
    glam::Vec3::new(-0.5, 0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, -0.5),
    glam::Vec3::new(-0.5, 0.5, -0.5),
    // bottom
    glam::Vec3::new(-0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, -0.5, -0.5),
    glam::Vec3::new(-0.5, -0.5, -0.5),
    // left
    glam::Vec3::new(-0.5, -0.5, 0.5),
    glam::Vec3::new(-0.5, 0.5, 0.5),
    glam::Vec3::new(-0.5, 0.5, -0.5),
    glam::Vec3::new(-0.5, -0.5, -0.5),
    // right
    glam::Vec3::new(0.5, -0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, 0.5),
    glam::Vec3::new(0.5, 0.5, -0.5),
    glam::Vec3::new(0.5, -0.5, -0.5),
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
const CUBE_WIREFRAME_INDICES: &[u32] = &[
    // front
    0, 1, 1, 2, 2, 3, 3, 0,
    // back
    4, 5, 5, 6, 6, 7, 7, 4,
    // top
    8, 9, 9, 10, 10, 11, 11, 8,
    // bottom
    12, 13, 13, 14, 14, 15, 15, 12,
    // left
    16, 17, 17, 18, 18, 19, 19, 16,
    // right
    20, 21, 21, 22, 22, 23, 23, 20,
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
    pub wireframe: bool,
}

impl CubePrimitive {
    pub fn new(side_length: f32, wireframe: bool) -> Self {
        Self {
            side_length,
            wireframe,
        }
    }
}

impl Primitive for CubePrimitive {
    fn generate_mesh(&self) -> Mesh {
        let mut vertices = Vec::with_capacity(CUBE_VERTICES.len());
        for i in 0..CUBE_VERTICES.len() {
            let position = CUBE_VERTICES[i] * self.side_length;
            let normal = CUBE_NORMALS[i];
            let tangent = glam::Vec3::ZERO;
            let tex_coords = CUBE_TEX_COORDS[i];

            vertices.push(Vertex {
                position,
                normal,
                tangent,
                tex_coords,
            });
        }

        let indices = if self.wireframe {
            CUBE_WIREFRAME_INDICES.to_vec()
        } else {
            CUBE_INDICES.to_vec()
        };

        // calculate_tangents(&mut vertices, &indices);

        Mesh::new(vertices, indices)
    }
}
