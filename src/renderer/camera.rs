/// Perspective 3d camera.
pub struct PerspectiveCamera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub aspect: f32,
    pub position: glam::Vec3,
    pub rotation: glam::Vec3,
}

impl PerspectiveCamera {
    pub fn new() -> Self {
        Self {
            fov: 60.0,
            near: 0.0001,
            far: 1000.0,
            aspect: 1.0,
            position: glam::Vec3::new(0.0, 0.0, 0.0),
            rotation: glam::Vec3::new(0.0, 0.0, 0.0),
        }
    }

    #[inline]
    pub fn get_view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::look_at_rh(self.position, glam::Vec3::ZERO, glam::Vec3::Y)
    }

    #[inline]
    pub fn get_projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fov.to_radians(), self.aspect, self.near, self.far)
    }

    #[inline]
    pub fn get_view_projection_matrix(&self) -> glam::Mat4 {
        self.get_projection_matrix() * self.get_view_matrix()
    }

    #[inline]
    pub fn get_view_matrix_inverse(&self) -> glam::Mat4 {
        self.get_view_matrix().inverse()
    }

    #[inline]
    pub fn get_projection_matrix_inverse(&self) -> glam::Mat4 {
        self.get_projection_matrix().inverse()
    }

    #[inline]
    pub fn get_view_projection_matrix_inverse(&self) -> glam::Mat4 {
        self.get_view_projection_matrix().inverse()
    }

    pub fn world_to_screen(&self, screen_size: (u32, u32), point: glam::Vec3) -> glam::Vec3 {
        let mut transformed_vertex = self.get_view_projection_matrix().transform_point3(point);

        transformed_vertex = glam::Vec3::new(
            transformed_vertex.x * screen_size.0 as f32 / 2.,
            transformed_vertex.y * screen_size.1 as f32 / 2.,
            transformed_vertex.z,
        );
        transformed_vertex +=
            glam::Vec3::new(screen_size.0 as f32 / 2., screen_size.1 as f32 / 2., 0.);

        transformed_vertex
    }
}

impl Default for PerspectiveCamera {
    fn default() -> Self {
        Self::new()
    }
}
