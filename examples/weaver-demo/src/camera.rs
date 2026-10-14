//! A free-flying camera driven by WASD and the right mouse button.

use weaver::prelude::*;

/// Component that moves a [`Camera`] around: WASD to move, Space/Left Ctrl for up/down, Left Shift
/// to go twice as fast, and mouse look while the right button is held.
///
/// It keeps its own position and orientation and writes the resulting view and projection matrices
/// into the entity's `Camera` every update.
#[derive(Debug, Clone, Copy)]
pub struct FlyCameraController {
    /// Movement speed in world units per second.
    pub speed: f32,
    /// Mouse look sensitivity in degrees per pixel of mouse movement.
    pub sensitivity: f32,
    /// Camera position in world space.
    pub translation: Vec3,
    /// Camera orientation.
    pub rotation: Quat,
    /// Vertical field of view in radians.
    pub fov: f32,
    /// Width over height; kept current by [`update_aspect_ratio`].
    pub aspect: f32,
    /// Near clip plane distance.
    pub near: f32,
    /// Far clip plane distance.
    pub far: f32,
}

impl Default for FlyCameraController {
    fn default() -> Self {
        Self {
            speed: 5.0,
            sensitivity: 0.1,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            fov: 60.0f32.to_radians(),
            aspect: 1.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl FlyCameraController {
    /// Applies one frame of input and updates `camera`'s matrices.
    pub fn update(&mut self, input: &Input, delta_time: f32, camera: &mut Camera) {
        let forward = self.rotation * Vec3::NEG_Z;
        let right = self.rotation * Vec3::X;

        let mut velocity = Vec3::ZERO;

        if input.key_down(KeyCode::KeyW) {
            velocity += forward;
        }
        if input.key_down(KeyCode::KeyS) {
            velocity -= forward;
        }
        if input.key_down(KeyCode::KeyD) {
            velocity += right;
        }
        if input.key_down(KeyCode::KeyA) {
            velocity -= right;
        }
        if input.key_down(KeyCode::Space) {
            velocity += Vec3::Y;
        }
        if input.key_down(KeyCode::ControlLeft) {
            velocity -= Vec3::Y;
        }

        velocity = velocity.normalize_or_zero() * self.speed * delta_time;

        if input.key_down(KeyCode::ShiftLeft) {
            velocity *= 2.0;
        }

        self.translation += velocity;

        if input.mouse_down(MouseButton::Right) {
            let (dx, dy) = input.mouse_delta();
            let (mut yaw, mut pitch, _roll) = self.rotation.to_euler(EulerRot::YXZ);

            // mouse delta is already a per-frame displacement, so it must not be scaled by delta_time
            yaw += -(dx * self.sensitivity).to_radians();
            pitch += -(dy * self.sensitivity).to_radians();

            pitch = pitch.clamp(
                -std::f32::consts::FRAC_PI_2 + 0.001,
                std::f32::consts::FRAC_PI_2 - 0.001,
            );

            self.rotation =
                Quat::from_axis_angle(Vec3::Y, yaw) * Quat::from_axis_angle(Vec3::X, pitch);
            self.rotation = self.rotation.normalize();
        }

        camera.set_view_projection_matrix(self.view_matrix(), self.projection_matrix());
    }

    /// World-to-camera matrix.
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.translation).inverse()
    }

    /// Perspective projection from the current fov, aspect ratio and clip planes.
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh_gl(self.fov, self.aspect, self.near, self.far)
    }

    /// Places the camera at `eye` facing `target`.
    pub fn looking_at(mut self, eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let matrix = Mat4::look_at_rh(eye, target, up).inverse();
        let (_scale, rotation, translation) = matrix.to_scale_rotation_translation();
        self.translation = translation;
        self.rotation = rotation;
        self
    }

    /// Moves the camera to `translation`.
    pub fn set_translation(&mut self, translation: Vec3) {
        self.translation = translation;
    }
}

/// Unused marker, apparently meant as a label for the camera systems.
pub struct CameraUpdate;

/// Runs every [`FlyCameraController`] against the input and frame time.
pub fn update_camera(
    time: Res<Time>,
    input: Res<Input>,
    mut query: Query<(&mut Camera, &mut FlyCameraController)>,
) {
    for (mut camera, mut controller) in query.iter() {
        controller.update(&input, time.delta_time, &mut camera);
    }
}

/// Keeps the controllers' aspect ratio matching the latest [`WindowResized`] event.
pub fn update_aspect_ratio(
    mut camera: Query<&mut FlyCameraController>,
    rx: EventRx<WindowResized>,
) {
    if let Some(event) = rx.last() {
        let WindowResized { width, height } = event;
        let aspect = width as f32 / height as f32;
        for mut camera in camera.iter() {
            camera.aspect = aspect;
        }
    }
}
