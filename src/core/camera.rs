use crate::ecs::resource::Resource;

pub struct PerspectiveCamera {
    pub view_matrix: glam::Mat4,
    pub projection_matrix: glam::Mat4,
    inverse_view_matrix: glam::Mat4,
}
impl Resource for PerspectiveCamera {}

impl PerspectiveCamera {
    pub fn new(
        position: glam::Vec3,
        look_at: glam::Vec3,
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let view_matrix = glam::Mat4::look_at_rh(position, look_at, glam::Vec3::NEG_Y);
        let projection_matrix = glam::Mat4::perspective_rh(fov, aspect, near, far);
        let inverse_view_matrix = view_matrix.inverse();

        Self {
            view_matrix,
            projection_matrix,
            inverse_view_matrix,
        }
    }

    pub fn position(&self) -> glam::Vec3 {
        self.inverse_view_matrix.col(3).truncate()
    }

    pub fn look_at(&mut self, eye: glam::Vec3, target: glam::Vec3, up: glam::Vec3) {
        self.view_matrix = glam::Mat4::look_at_rh(eye, target, up);
        self.inverse_view_matrix = self.view_matrix.inverse();
    }

    pub fn world_to_projection(&self, position: glam::Vec3) -> glam::Vec3 {
        let view = self.view_matrix.transform_point3(position);
        self.projection_matrix.transform_point3(view)
    }
}
