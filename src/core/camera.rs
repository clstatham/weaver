pub struct PerspectiveCamera {
    pub view_matrix: glam::Mat4,
    pub projection_matrix: glam::Mat4,
    inverse_view_matrix: glam::Mat4,
    inverse_projection_matrix: glam::Mat4,
}

impl PerspectiveCamera {
    pub fn new(
        position: glam::Vec3A,
        look_at: glam::Vec3A,
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let view_matrix =
            glam::Mat4::look_at_rh(position.into(), look_at.into(), glam::Vec3::NEG_Y);
        let projection_matrix = glam::Mat4::perspective_rh(fov, aspect, near, far);
        let inverse_view_matrix = view_matrix.inverse();
        let inverse_projection_matrix = projection_matrix.inverse();

        Self {
            view_matrix,
            projection_matrix,
            inverse_view_matrix,
            inverse_projection_matrix,
        }
    }

    pub fn position(&self) -> glam::Vec3A {
        self.inverse_view_matrix.col(3).truncate().into()
    }

    pub fn look_at(&mut self, eye: glam::Vec3A, target: glam::Vec3A, up: glam::Vec3A) {
        self.view_matrix = glam::Mat4::look_at_rh(eye.into(), target.into(), up.into());
        self.inverse_view_matrix = self.view_matrix.inverse();
    }

    pub fn world_to_projection(&self, position: glam::Vec3A) -> glam::Vec3A {
        let view = self.view_matrix.transform_point3(position.into());
        self.projection_matrix.transform_point3(view).into()
    }
}
