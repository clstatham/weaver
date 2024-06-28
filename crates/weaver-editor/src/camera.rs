use weaver::prelude::*;

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(ReflectComponent)]
pub struct FlyCameraController {
    pub speed: f32,
    pub sensitivity: f32,
    pub translation: Vec3,
    pub rotation: Quat,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
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
            let mouse_delta = input.mouse_delta();
            let (mut yaw, mut pitch, _roll) = self.rotation.to_euler(EulerRot::YXZ);

            yaw += -(mouse_delta.0 * self.sensitivity).to_radians();
            pitch += -(mouse_delta.1 * self.sensitivity).to_radians();

            pitch = pitch.clamp(
                -std::f32::consts::FRAC_PI_2 + 0.001,
                std::f32::consts::FRAC_PI_2 - 0.001,
            );

            self.rotation =
                Quat::from_axis_angle(Vec3::Y, yaw) * Quat::from_axis_angle(Vec3::X, pitch);
            self.rotation = self.rotation.normalize();
        }

        camera.view_matrix = self.view_matrix();
        camera.projection_matrix = self.projection_matrix();
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.translation).inverse()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    pub fn look_at(&mut self, eye: Vec3, target: Vec3, up: Vec3) -> &mut Self {
        let matrix = Mat4::look_at_rh(eye, target, up).inverse();
        let (_scale, rotation, translation) = matrix.to_scale_rotation_translation();
        self.translation = translation;
        self.rotation = rotation;
        self
    }

    pub fn set_translation(&mut self, translation: Vec3) {
        self.translation = translation;
    }
}

pub fn update_camera(
    time: Res<Time>,
    input: Res<Input>,
    query: Query<(&mut Camera, &mut FlyCameraController)>,
) -> Result<()> {
    for (_entity, (mut camera, mut controller)) in query.iter() {
        controller.update(&input, time.delta_time, &mut camera);
    }

    Ok(())
}

pub fn update_aspect_ratio(
    camera: Query<&mut FlyCameraController>,
    rx: EventRx<WindowResized>,
) -> Result<()> {
    let events: Vec<_> = rx.iter().collect();
    if let Some(event) = events.last() {
        let WindowResized { width, height } = **event;
        let aspect = width as f32 / height as f32;
        for (_entity, mut camera) in camera.iter() {
            camera.aspect = aspect;
        }
    }

    Ok(())
}
