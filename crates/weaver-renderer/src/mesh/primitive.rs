use glam::{Vec2, Vec3};
use weaver_core::{
    mesh::{Mesh, Vertex},
    transform::Transform,
};

pub trait Primitive {
    fn generate_mesh(&self) -> Mesh;
}

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

pub fn create_unit_cube(wireframe: bool) -> Mesh {
    let vertices = vec![
        // Front face
        Vertex {
            position: Vec3::new(-1.0, -1.0, 1.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(0.0, 0.0),
        },
        Vertex {
            position: Vec3::new(1.0, -1.0, 1.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(1.0, 0.0),
        },
        Vertex {
            position: Vec3::new(1.0, 1.0, 1.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(1.0, 1.0),
        },
        Vertex {
            position: Vec3::new(-1.0, 1.0, 1.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(0.0, 1.0),
        },
        // Back face
        Vertex {
            position: Vec3::new(-1.0, -1.0, -1.0),
            normal: Vec3::new(0.0, 0.0, -1.0),
            tangent: Vec3::new(-1.0, 0.0, 0.0),
            tex_coords: Vec2::new(1.0, 0.0),
        },
        Vertex {
            position: Vec3::new(1.0, -1.0, -1.0),
            normal: Vec3::new(0.0, 0.0, -1.0),
            tangent: Vec3::new(-1.0, 0.0, 0.0),
            tex_coords: Vec2::new(0.0, 0.0),
        },
        Vertex {
            position: Vec3::new(1.0, 1.0, -1.0),
            normal: Vec3::new(0.0, 0.0, -1.0),
            tangent: Vec3::new(-1.0, 0.0, 0.0),
            tex_coords: Vec2::new(0.0, 1.0),
        },
        Vertex {
            position: Vec3::new(-1.0, 1.0, -1.0),
            normal: Vec3::new(0.0, 0.0, -1.0),
            tangent: Vec3::new(-1.0, 0.0, 0.0),
            tex_coords: Vec2::new(1.0, 1.0),
        },
        // Top face
        Vertex {
            position: Vec3::new(-1.0, 1.0, -1.0),
            normal: Vec3::new(0.0, 1.0, 0.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(0.0, 0.0),
        },
        Vertex {
            position: Vec3::new(1.0, 1.0, -1.0),
            normal: Vec3::new(0.0, 1.0, 0.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(1.0, 0.0),
        },
        Vertex {
            position: Vec3::new(1.0, 1.0, 1.0),
            normal: Vec3::new(0.0, 1.0, 0.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(1.0, 1.0),
        },
        Vertex {
            position: Vec3::new(-1.0, 1.0, 1.0),
            normal: Vec3::new(0.0, 1.0, 0.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(0.0, 1.0),
        },
        // Bottom face
        Vertex {
            position: Vec3::new(-1.0, -1.0, -1.0),
            normal: Vec3::new(0.0, -1.0, 0.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(0.0, 1.0),
        },
        Vertex {
            position: Vec3::new(1.0, -1.0, -1.0),
            normal: Vec3::new(0.0, -1.0, 0.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(1.0, 1.0),
        },
        Vertex {
            position: Vec3::new(1.0, -1.0, 1.0),
            normal: Vec3::new(0.0, -1.0, 0.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(1.0, 0.0),
        },
        Vertex {
            position: Vec3::new(-1.0, -1.0, 1.0),
            normal: Vec3::new(0.0, -1.0, 0.0),
            tangent: Vec3::new(1.0, 0.0, 0.0),
            tex_coords: Vec2::new(0.0, 0.0),
        },
        // Right face
        Vertex {
            position: Vec3::new(1.0, -1.0, -1.0),
            normal: Vec3::new(1.0, 0.0, 0.0),
            tangent: Vec3::new(0.0, 0.0, 1.0),
            tex_coords: Vec2::new(0.0, 0.0),
        },
        Vertex {
            position: Vec3::new(1.0, 1.0, -1.0),
            normal: Vec3::new(1.0, 0.0, 0.0),
            tangent: Vec3::new(0.0, 0.0, 1.0),
            tex_coords: Vec2::new(0.0, 1.0),
        },
        Vertex {
            position: Vec3::new(1.0, 1.0, 1.0),
            normal: Vec3::new(1.0, 0.0, 0.0),
            tangent: Vec3::new(0.0, 0.0, 1.0),
            tex_coords: Vec2::new(1.0, 1.0),
        },
        Vertex {
            position: Vec3::new(1.0, -1.0, 1.0),
            normal: Vec3::new(1.0, 0.0, 0.0),
            tangent: Vec3::new(0.0, 0.0, 1.0),
            tex_coords: Vec2::new(1.0, 0.0),
        },
        // Left face
        Vertex {
            position: Vec3::new(-1.0, -1.0, -1.0),
            normal: Vec3::new(-1.0, 0.0, 0.0),
            tangent: Vec3::new(0.0, 0.0, -1.0),
            tex_coords: Vec2::new(1.0, 0.0),
        },
        Vertex {
            position: Vec3::new(-1.0, 1.0, -1.0),
            normal: Vec3::new(-1.0, 0.0, 0.0),
            tangent: Vec3::new(0.0, 0.0, -1.0),
            tex_coords: Vec2::new(1.0, 1.0),
        },
        Vertex {
            position: Vec3::new(-1.0, 1.0, 1.0),
            normal: Vec3::new(-1.0, 0.0, 0.0),
            tangent: Vec3::new(0.0, 0.0, -1.0),
            tex_coords: Vec2::new(0.0, 1.0),
        },
        Vertex {
            position: Vec3::new(-1.0, -1.0, 1.0),
            normal: Vec3::new(-1.0, 0.0, 0.0),
            tangent: Vec3::new(0.0, 0.0, -1.0),
            tex_coords: Vec2::new(0.0, 0.0),
        },
    ];

    #[rustfmt::skip]
    let indices = if wireframe {
        vec![
            // Front face
            0, 1, 1, 2, 2, 3, 3, 0, 
            // Back face
            4, 5, 5, 6, 6, 7, 7, 4, 
            // Top face
            8, 9, 9, 10, 10, 11, 11, 8, 
            // Bottom face
            12, 13, 13, 14, 14, 15, 15, 12, 
            // Right face
            16, 17, 17, 18, 18, 19, 19, 16, 
            // Left face
            20, 21, 21, 22, 22, 23, 23, 20,
        ]
    } else {
        vec![
            // Front face
            0, 1, 2, 2, 3, 0, 
            // Back face
            4, 5, 6, 6, 7, 4, 
            // Top face
            8, 9, 10, 10, 11, 8, 
            // Bottom face
            12, 13, 14, 14, 15, 12, 
            // Right face
            16, 17, 18, 18, 19, 16, 
            // Left face
            20, 21, 22, 22, 23, 20,
        ]
    };

    Mesh::new(vertices, indices)
}

impl Primitive for CubePrimitive {
    fn generate_mesh(&self) -> Mesh {
        create_unit_cube(self.wireframe)
            .transformed(Transform::from_scale(Vec3::splat(self.side_length / 2.0)))
    }
}
