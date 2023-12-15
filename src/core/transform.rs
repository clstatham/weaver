use super::Vertex;

#[derive(Debug, Clone, Copy)]
pub struct Transform(pub glam::Mat4);

impl Transform {
    pub fn new() -> Self {
        Self(glam::Mat4::IDENTITY)
    }

    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        self.0 *= glam::Mat4::from_translation(glam::Vec3::new(x, y, z));
    }

    pub fn rotate(&mut self, angle: f32, axis: glam::Vec3) {
        self.0 *= glam::Mat4::from_axis_angle(axis, angle);
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) {
        self.0 *= glam::Mat4::from_scale(glam::Vec3::new(x, y, z));
    }

    pub fn look_at(&mut self, eye: glam::Vec3, target: glam::Vec3, up: glam::Vec3) {
        self.0 = glam::Mat4::look_at_rh(eye, target, up);
    }

    pub fn perspective(&mut self, fov: f32, aspect: f32, near: f32, far: f32) {
        self.0 = glam::Mat4::perspective_rh(fov, aspect, near, far);
    }

    pub fn orthographic(
        &mut self,
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    ) {
        self.0 = glam::Mat4::orthographic_rh(left, right, bottom, top, near, far);
    }

    pub fn transform_vertex(&self, vertex: Vertex) -> Vertex {
        let position = self.0.transform_point3(vertex.position);
        let normal = self.0.transform_vector3(vertex.normal).normalize();
        let color = vertex.color;

        Vertex {
            position,
            normal,
            color,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}
