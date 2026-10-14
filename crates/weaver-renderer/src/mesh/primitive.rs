//! Generated meshes for quick scenes and debugging.

use glam::{Vec2, Vec3};
use weaver_core::mesh::{Mesh, Vertex};

/// A shape that can build a [`Mesh`] for itself.
pub trait Primitive {
    /// Builds the mesh.
    fn generate_mesh(&self) -> Mesh;
}

/// An axis-aligned cube centered on the origin.
pub struct CubePrimitive {
    /// Edge length.
    pub side_length: f32,
    /// If set, the mesh is the 12 edges as a line list rather than 36 triangle indices.
    pub wireframe: bool,
}

impl CubePrimitive {
    /// Creates a cube with the given edge length.
    pub fn new(side_length: f32, wireframe: bool) -> Self {
        Self {
            side_length,
            wireframe,
        }
    }
}

/// Builds a cube spanning -1 to 1 on each axis (so edge length 2), with 24 vertices.
///
/// With `wireframe` the indices are a line list of the 12 edges, otherwise a triangle list.
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
            4, 7, 6, 6, 5, 4, 
            // Top face
            8, 11, 10, 10, 9, 8, 
            // Bottom face
            12, 13, 14, 14, 15, 12, 
            // Right face
            16, 17, 18, 18, 19, 16, 
            // Left face
            20, 23, 22, 22, 21, 20,
        ]
    };

    Mesh::new(vertices, indices)
}

impl Primitive for CubePrimitive {
    fn generate_mesh(&self) -> Mesh {
        let mut mesh = create_unit_cube(self.wireframe);
        for vertex in mesh.vertices.iter_mut() {
            vertex.position *= self.side_length / 2.0;
        }
        mesh.regenerate_aabb();
        mesh
    }
}
