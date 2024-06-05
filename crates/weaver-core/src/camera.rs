use std::fmt::Debug;

use crate::{
    geom::Ray,
    renderer::internals::{GpuResourceType, LazyBindGroup, LazyGpuHandle},
};

use weaver_ecs::world::World;
use weaver_proc_macro::{BindableComponent, GpuComponent};
use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;

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

#[derive(Clone, GpuComponent, BindableComponent)]
#[gpu(update = "update")]
pub struct Camera {
    pub view_matrix: glam::Mat4,
    pub projection_matrix: glam::Mat4,

    #[uniform]
    pub(crate) handle: LazyGpuHandle,
    pub(crate) bind_group: LazyBindGroup<Self>,
}

impl Debug for Camera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Camera")
            .field("view_matrix", &self.view_matrix)
            .field("projection_matrix", &self.projection_matrix)
            .finish()
    }
}

impl Camera {
    pub fn new(view_matrix: glam::Mat4, projection_matrix: glam::Mat4) -> Self {
        Self {
            view_matrix,
            projection_matrix,
            handle: Self::default_handle(),
            bind_group: LazyBindGroup::default(),
        }
    }

    pub fn screen_to_ray(&self, screen_position: glam::Vec2, screen_size: glam::Vec2) -> Ray {
        let ndc = glam::Vec2::new(
            (screen_position.x / screen_size.x) * 2.0 - 1.0,
            -((screen_position.y / screen_size.y) * 2.0 - 1.0),
        );

        let inv_proj = self.projection_matrix.inverse();
        let inv_view = self.view_matrix.inverse();

        let clip = glam::Vec4::new(ndc.x, ndc.y, -1.0, 1.0);
        let eye = inv_proj * clip;
        let eye = glam::Vec4::new(eye.x, eye.y, -1.0, 0.0);
        let world = inv_view * eye;

        let world = world.truncate().normalize();

        Ray {
            origin: inv_view.col(3).truncate(),
            direction: world,
        }
    }

    pub fn perspective_lookat(
        eye: glam::Vec3,
        center: glam::Vec3,
        up: glam::Vec3,
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self::new(
            glam::Mat4::look_at_rh(eye, center, up),
            glam::Mat4::perspective_rh(fov, aspect, near, far),
        )
    }

    fn default_handle() -> LazyGpuHandle {
        LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                size: std::mem::size_of::<CameraUniform>(),
            },
            Some("Camera"),
            None,
        )
    }

    pub fn update(&self, _world: &World) -> anyhow::Result<()> {
        self.handle.update(&[CameraUniform::from(self)]);
        Ok(())
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(glam::Mat4::IDENTITY, glam::Mat4::IDENTITY)
    }
}

#[derive(Debug, Clone, Copy)]
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
    pub fn update(&mut self, input: &Input, delta_time: f32, aspect: f32, camera: &mut Camera) {
        if input.mouse_button_pressed(MouseButton::Right) {
            let mouse_delta = input.mouse_delta();
            let (mut yaw, mut pitch, _roll) = self.rotation.to_euler(glam::EulerRot::YXZ);

            let forward = self.rotation * glam::Vec3::NEG_Z;
            let right = self.rotation * glam::Vec3::X;

            let mut velocity = glam::Vec3::ZERO;

            if input.key_pressed(KeyCode::KeyW) {
                velocity += forward;
            }
            if input.key_pressed(KeyCode::KeyS) {
                velocity -= forward;
            }
            if input.key_pressed(KeyCode::KeyD) {
                velocity += right;
            }
            if input.key_pressed(KeyCode::KeyA) {
                velocity -= right;
            }
            if input.key_pressed(KeyCode::Space) {
                velocity += glam::Vec3::Y;
            }
            if input.key_pressed(KeyCode::ControlLeft) {
                velocity -= glam::Vec3::Y;
            }

            velocity = velocity.normalize_or_zero() * self.speed * delta_time;

            if input.key_pressed(KeyCode::ShiftLeft) {
                velocity *= 2.0;
            }

            self.translation += velocity;

            yaw += -(mouse_delta.x * self.sensitivity).to_radians();
            pitch += -(mouse_delta.y * self.sensitivity).to_radians();

            pitch = pitch.clamp(
                -std::f32::consts::FRAC_PI_2 + 0.001,
                std::f32::consts::FRAC_PI_2 - 0.001,
            );

            self.rotation = glam::Quat::from_axis_angle(glam::Vec3::Y, yaw)
                * glam::Quat::from_axis_angle(glam::Vec3::X, pitch);
            self.rotation = self.rotation.normalize();
        }

        self.aspect = aspect;
        camera.view_matrix = self.view_matrix();
        camera.projection_matrix = self.projection_matrix();
    }

    pub fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_rotation_translation(self.rotation, self.translation).inverse()
    }

    pub fn projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    pub fn set_translation(&mut self, translation: glam::Vec3) {
        self.translation = translation;
    }
}

impl Default for FlyCameraController {
    fn default() -> Self {
        Self {
            speed: 10.0,
            sensitivity: 0.1,
            translation: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            fov: 60.0f32.to_radians(),
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
        }
    }
}
