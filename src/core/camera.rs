use weaver_proc_macro::Resource;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    pub view_proj: glam::Mat4,
    pub inv_view_proj: glam::Mat4,
    pub camera_position: glam::Vec3,
    _padding: u32,
}

impl From<Camera> for CameraUniform {
    fn from(camera: Camera) -> Self {
        let view_proj = camera.projection_matrix() * camera.view_matrix();
        let inv_view_proj = view_proj.inverse();
        let camera_position = camera.eye;

        Self {
            view_proj,
            inv_view_proj,
            camera_position,
            _padding: 0,
        }
    }
}

#[derive(Debug, Resource, Clone, Copy)]
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

    pub fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::look_at_rh(self.eye, self.target, self.up)
    }

    pub fn projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh_gl(self.fov, self.aspect, self.near, self.far)
    }
}
