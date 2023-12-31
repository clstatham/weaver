use weaver_proc_macro::Resource;
use winit::event::VirtualKeyCode;

use super::input::Input;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub inv_view: glam::Mat4,
    pub inv_proj: glam::Mat4,
    pub camera_position: glam::Vec3,
    _padding: u32,
}

impl From<Camera> for CameraUniform {
    fn from(camera: Camera) -> Self {
        let view = camera.view_matrix();
        let proj = camera.projection_matrix();
        let inv_view = view.inverse();
        let inv_proj = proj.inverse();
        let camera_position = camera.eye;

        Self {
            view,
            proj,
            inv_view,
            inv_proj,
            camera_position,
            _padding: 0,
        }
    }
}

impl From<FlyCamera> for CameraUniform {
    fn from(camera: FlyCamera) -> Self {
        let view = camera.view_matrix();
        let proj = camera.projection_matrix();
        let inv_view = view.inverse();
        let inv_proj = proj.inverse();
        let camera_position = camera.translation;

        Self {
            view,
            proj,
            inv_view,
            inv_proj,
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

#[derive(Debug, Resource, Clone, Copy)]
pub struct FlyCamera {
    pub speed: f32,
    pub sensitivity: f32,
    pub translation: glam::Vec3,
    pub rotation: glam::Quat,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl FlyCamera {
    pub fn update(&mut self, input: &Input, delta_time: f32) {
        let mouse_delta = input.mouse_delta();
        let (mut yaw, mut pitch, _roll) = self.rotation.to_euler(glam::EulerRot::YXZ);

        let forward = self.rotation * glam::Vec3::NEG_Z;
        let right = self.rotation * glam::Vec3::X;

        let mut velocity = glam::Vec3::ZERO;

        if input.is_key_pressed(VirtualKeyCode::W) {
            velocity += forward;
        }
        if input.is_key_pressed(VirtualKeyCode::S) {
            velocity -= forward;
        }
        if input.is_key_pressed(VirtualKeyCode::D) {
            velocity += right;
        }
        if input.is_key_pressed(VirtualKeyCode::A) {
            velocity -= right;
        }
        if input.is_key_pressed(VirtualKeyCode::Space) {
            velocity += glam::Vec3::Y;
        }
        if input.is_key_pressed(VirtualKeyCode::LControl) {
            velocity -= glam::Vec3::Y;
        }

        velocity = velocity.normalize_or_zero() * self.speed * delta_time;

        if input.is_key_pressed(VirtualKeyCode::LShift) {
            velocity *= 2.0;
        }

        self.translation += velocity;

        if input.is_mouse_button_pressed(winit::event::MouseButton::Left) {
            yaw += -(mouse_delta.x * self.sensitivity).to_radians();
            pitch += -(mouse_delta.y * self.sensitivity).to_radians();
        }

        pitch = pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.001,
            std::f32::consts::FRAC_PI_2 - 0.001,
        );

        self.rotation = glam::Quat::from_axis_angle(glam::Vec3::Y, yaw)
            * glam::Quat::from_axis_angle(glam::Vec3::X, pitch);
        self.rotation = self.rotation.normalize();
    }

    fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_rotation_translation(self.rotation, self.translation).inverse()
    }

    pub fn projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh_gl(self.fov, self.aspect, self.near, self.far)
    }
}
