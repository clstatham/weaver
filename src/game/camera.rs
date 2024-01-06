use crate::{core::camera::CameraUniform, prelude::*};

use super::player::Player;

#[derive(Debug, Clone, Copy, Resource)]
pub struct FollowCamera {
    pub target: Entity,
    pub rotation: glam::Quat,

    pub pitch_sensitivity: f32,

    pub distance: f32,
    pub max_distance: f32,
    pub min_distance: f32,

    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,

    pub translation: glam::Vec3,
    pub target_translation: glam::Vec3,

    pub pitch: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,

    pub stiffness: f32,
}

impl Default for FollowCamera {
    fn default() -> Self {
        Self {
            target: Entity::PLACEHOLDER,
            rotation: glam::Quat::IDENTITY,
            fov: std::f32::consts::FRAC_PI_2,
            pitch_sensitivity: 0.5,
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
            min_pitch: -std::f32::consts::FRAC_PI_2 + 0.001,
            max_pitch: std::f32::consts::FRAC_PI_2 - 0.001,
            translation: glam::Vec3::new(0.0, 5.0, -5.0),
            target_translation: glam::Vec3::ZERO,
            pitch: 0.0,
            stiffness: 2.0,
            distance: 5.0,
            max_distance: 10.0,
            min_distance: 1.0,
        }
    }
}

impl FollowCamera {
    pub fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::look_at_rh(self.translation, self.target_translation, glam::Vec3::Y)
    }

    pub fn projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }
}

impl From<FollowCamera> for CameraUniform {
    fn from(camera: FollowCamera) -> Self {
        Self {
            view: camera.view_matrix(),
            proj: camera.projection_matrix(),
            inv_view: camera.view_matrix().inverse(),
            inv_proj: camera.projection_matrix().inverse(),
            camera_position: camera.translation,
            _padding: 0,
        }
    }
}

#[system(FollowCameraUpdate)]
pub fn follow_camera_update(
    mut camera: ResMut<FollowCamera>,
    time: Res<Time>,
    input: Res<Input>,
    player_transform: Query<&Transform, With<Player>>,
) {
    let player_transform = player_transform.get(camera.target).unwrap();
    let player_translation = player_transform.get_translation();
    let player_rotation = player_transform.get_rotation();

    if input.mouse_button_pressed(MouseButton::Right) {
        let mouse_delta = input.mouse_delta();
        camera.pitch += mouse_delta.y * camera.pitch_sensitivity * time.delta_time;
        camera.pitch = camera.pitch.clamp(camera.min_pitch, camera.max_pitch);
    }

    camera.distance -= input.mouse_wheel_delta() * 0.5;
    camera.distance = camera
        .distance
        .clamp(camera.min_distance, camera.max_distance);

    camera.rotation = glam::Quat::from_rotation_x(camera.pitch);

    let rotation = player_rotation * camera.rotation;
    let translation = player_translation + rotation * glam::Vec3::NEG_Z * camera.distance;

    camera.translation = camera
        .translation
        .lerp(translation, camera.stiffness * time.delta_time);
    camera.target_translation = player_translation;
}
