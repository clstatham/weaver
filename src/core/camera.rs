use weaver_proc_macro::Component;
use winit::event::VirtualKeyCode;

use crate::renderer::{
    AllocBuffers, BufferHandle, CreateBindGroupLayout, LazyBufferHandle, Renderer,
};

use super::input::Input;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub inv_view: glam::Mat4,
    pub inv_proj: glam::Mat4,
    pub camera_position: glam::Vec3,
    pub _padding: u32,
}

impl From<&Camera> for CameraUniform {
    fn from(camera: &Camera) -> Self {
        let view = camera.view_matrix;
        let proj = camera.projection_matrix;
        let inv_view = view.inverse();
        let inv_proj = proj.inverse();
        let camera_position = inv_view.col(3).truncate();

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

#[derive(Clone, Component)]
pub struct Camera {
    pub view_matrix: glam::Mat4,
    pub projection_matrix: glam::Mat4,

    pub(crate) handle: LazyBufferHandle,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            view_matrix: glam::Mat4::IDENTITY,
            projection_matrix: glam::Mat4::IDENTITY,
            handle: LazyBufferHandle::new(
                crate::renderer::BufferBindingType::Uniform {
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    size: Some(std::mem::size_of::<CameraUniform>()),
                },
                Some("Camera"),
                None,
            ),
        }
    }

    pub fn update(&mut self) {
        self.handle
            .update::<CameraUniform>(&[CameraUniform::from(&*self)]);
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

impl AllocBuffers for Camera {
    fn alloc_buffers(&self, renderer: &Renderer) -> anyhow::Result<Vec<BufferHandle>> {
        Ok(vec![self.handle.get_or_create_init::<_, Self>(
            renderer,
            &[CameraUniform::from(self)],
        )])
    }
}

impl CreateBindGroupLayout for Camera {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }
}

#[derive(Debug, Component, Clone, Copy)]
pub struct FlyCameraController {
    pub speed: f32,
    pub sensitivity: f32,
    pub translation: glam::Vec3,
    pub rotation: glam::Quat,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl FlyCameraController {
    pub fn update(&mut self, input: &Input, delta_time: f32, camera: &mut Camera) {
        let mouse_delta = input.mouse_delta();
        let (mut yaw, mut pitch, _roll) = self.rotation.to_euler(glam::EulerRot::YXZ);

        let forward = self.rotation * glam::Vec3::NEG_Z;
        let right = self.rotation * glam::Vec3::X;

        let mut velocity = glam::Vec3::ZERO;

        if input.key_pressed(VirtualKeyCode::W) {
            velocity += forward;
        }
        if input.key_pressed(VirtualKeyCode::S) {
            velocity -= forward;
        }
        if input.key_pressed(VirtualKeyCode::D) {
            velocity += right;
        }
        if input.key_pressed(VirtualKeyCode::A) {
            velocity -= right;
        }
        if input.key_pressed(VirtualKeyCode::Space) {
            velocity += glam::Vec3::Y;
        }
        if input.key_pressed(VirtualKeyCode::LControl) {
            velocity -= glam::Vec3::Y;
        }

        velocity = velocity.normalize_or_zero() * self.speed * delta_time;

        if input.key_pressed(VirtualKeyCode::LShift) {
            velocity *= 2.0;
        }

        self.translation += velocity;

        if input.mouse_button_pressed(3) {
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

        camera.view_matrix = self.view_matrix();
        camera.projection_matrix = self.projection_matrix();

        camera.update();
    }

    pub fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_rotation_translation(self.rotation, self.translation).inverse()
    }

    pub fn projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }
}
