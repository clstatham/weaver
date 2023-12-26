#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    pub view_projection_matrix: glam::Mat4,
    pub camera_position: glam::Vec4,
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_projection_matrix: glam::Mat4::IDENTITY,
            camera_position: glam::Vec4::ZERO,
        }
    }

    pub fn update(&mut self, camera: &Camera) {
        self.view_projection_matrix = camera.view_projection_matrix();
        self.camera_position = camera.eye.extend(1.0);
    }
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Camera {
    pub eye: glam::Vec3,
    pub target: glam::Vec3,
    pub up: glam::Vec3,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn new(
        eye: glam::Vec3,
        target: glam::Vec3,
        up: glam::Vec3,
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            eye,
            target,
            up,
            fov,
            aspect,
            near,
            far,
        }
    }

    pub fn view_projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh_gl(self.fov, self.aspect, self.near, self.far)
            * glam::Mat4::look_at_rh(self.eye, self.target, self.up)
    }
}
