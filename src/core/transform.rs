use super::Vertex;

#[derive(Debug, Clone, Copy, PartialEq, weaver_proc_macro::Component)]
pub struct Transform {
    pub matrix: glam::Mat4,
}

impl Transform {
    pub fn new() -> Self {
        Self {
            matrix: glam::Mat4::IDENTITY,
        }
    }

    pub fn translate(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.matrix *= glam::Mat4::from_translation(glam::Vec3::new(x, y, z));
        *self
    }

    pub fn rotate(&mut self, angle: f32, axis: glam::Vec3) -> Self {
        self.matrix *= glam::Mat4::from_axis_angle(axis, angle);
        *self
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) -> Self {
        self.matrix *= glam::Mat4::from_scale(glam::Vec3::new(x, y, z));
        *self
    }

    pub fn transform_vertex(&self, vertex: Vertex) -> Vertex {
        let position = self.matrix.transform_point3(vertex.position);
        let normal = self.matrix.transform_vector3(vertex.normal).normalize();
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
