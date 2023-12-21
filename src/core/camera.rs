pub struct PerspectiveCamera {
    pub view_matrix: glam::Mat4,
    pub projection_matrix: glam::Mat4,
}

impl PerspectiveCamera {
    pub fn new(
        position: glam::Vec3,
        look_at: glam::Vec3,
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            view_matrix: glam::Mat4::look_at_rh(position, look_at, glam::Vec3::NEG_Y),
            projection_matrix: glam::Mat4::perspective_rh(fov, aspect, near, far),
        }
    }

    pub fn look_at(&mut self, eye: glam::Vec3, target: glam::Vec3, up: glam::Vec3) {
        self.view_matrix = glam::Mat4::look_at_rh(eye, target, up);
    }

    pub fn world_to_projection(&self, position: glam::Vec3) -> glam::Vec3 {
        let view = self.view_matrix.transform_point3(position);
        self.projection_matrix.transform_point3(view)
    }
}
