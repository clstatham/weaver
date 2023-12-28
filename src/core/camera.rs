use weaver_proc_macro::Resource;

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
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Resource)]
pub struct Camera {
    pub eye: glam::Vec3,
    pub target: glam::Vec3,
    pub up: glam::Vec3,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,

    pub(crate) uniform: CameraUniform,
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
        let mut this = Self {
            eye,
            target,
            up,
            fov,
            aspect,
            near,
            far,
            uniform: CameraUniform::new(),
        };
        this.uniform.view_projection_matrix = this.view_projection_matrix();
        this.uniform.camera_position = this.eye.extend(1.0);
        this
    }

    pub fn update(&mut self) {
        self.uniform.view_projection_matrix = self.view_projection_matrix();
        self.uniform.camera_position = self.eye.extend(1.0);
    }

    pub fn view_projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh_gl(self.fov, self.aspect, self.near, self.far)
            * glam::Mat4::look_at_rh(self.eye, self.target, self.up)
    }
}
