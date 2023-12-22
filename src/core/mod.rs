use self::color::Color;

pub mod camera;
pub mod color;
pub mod input;
pub mod light;
pub mod mesh;
pub mod transform;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
    pub color: Color,
}

impl Vertex {
    pub fn new(position: glam::Vec3, normal: glam::Vec3, color: Color) -> Self {
        Self {
            position,
            normal,
            color,
        }
    }
}
