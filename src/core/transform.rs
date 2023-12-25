use crate::ecs::component::Component;

use super::Vertex;

#[derive(Debug, Clone, Copy)]
pub struct Transform(pub glam::Mat4);
impl Component for Transform {}

impl Transform {
    pub fn new() -> Self {
        Self(glam::Mat4::IDENTITY)
    }

    pub fn translate(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.0 *= glam::Mat4::from_translation(glam::Vec3::new(x, y, z));
        *self
    }

    pub fn rotate(&mut self, angle: f32, axis: glam::Vec3A) -> Self {
        self.0 *= glam::Mat4::from_axis_angle(axis.into(), angle);
        *self
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.0 *= glam::Mat4::from_scale(glam::Vec3::new(x, y, z));
        *self
    }

    pub fn transform_vertex(&self, vertex: Vertex) -> Vertex {
        let position = self.0.transform_point3a(vertex.position);
        let normal = self.0.transform_vector3a(vertex.normal).normalize();
        let color = vertex.color;
        let uv = vertex.uv;

        Vertex {
            position,
            normal,
            color,
            uv,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}
