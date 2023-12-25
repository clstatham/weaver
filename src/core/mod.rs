use self::color::Color;

pub mod camera;
pub mod color;
pub mod input;
pub mod light;
pub mod mesh;
pub mod texture;
pub mod transform;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vertex {
    pub position: glam::Vec3A,
    pub normal: glam::Vec3A,
    pub color: Color,
    pub uv: glam::Vec2,
}

impl Vertex {
    pub fn new(position: glam::Vec3A, normal: glam::Vec3A, color: Color, uv: glam::Vec2) -> Self {
        Self {
            position,
            normal,
            color,
            uv,
        }
    }
}
