use crate::prelude::*;

#[derive(Debug, Clone, Copy, Component)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Player {
    pub speed: f32,
    pub rotation_speed: f32,
    pub velocity: Vec3,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            speed: 10.0,
            rotation_speed: 1.0,
            velocity: Vec3::ZERO,
        }
    }
}

#[derive(Bundle)]
pub struct PlayerBundle {
    pub player: Player,
    pub transform: Transform,
    pub mesh: Mesh,
    pub material: Material,
}

#[system(PlayerInput)]
pub fn player_update(mut player: Query<(&mut Player, &mut Transform)>, input: Res<Input>) {
    for (mut player, mut transform) in player.iter() {
        let mut translation = transform.get_translation();
        let mut rotation = transform.get_rotation();

        let mouse_delta = input.mouse_delta();

        if input.mouse_button_pressed(MouseButton::Right) {
            let delta = mouse_delta * player.rotation_speed * 0.005;
            rotation = Quat::from_rotation_y(-delta.x) * rotation;
        }

        let mut direction = Vec3::ZERO;

        if input.key_pressed(KeyCode::KeyW) {
            direction += rotation * Vec3::Z;
        }
        if input.key_pressed(KeyCode::KeyS) {
            direction -= rotation * Vec3::Z;
        }
        if input.key_pressed(KeyCode::KeyA) {
            direction += rotation * Vec3::X;
        }
        if input.key_pressed(KeyCode::KeyD) {
            direction -= rotation * Vec3::X;
        }
        if input.key_pressed(KeyCode::Space) {
            direction += rotation * Vec3::Y;
        }
        if input.key_pressed(KeyCode::ControlLeft) {
            direction -= rotation * Vec3::Y;
        }

        player.velocity = direction.normalize_or_zero() * player.speed;

        transform.set_translation(translation);
        transform.set_rotation(rotation);
    }
}

#[system(PlayerMovement)]
pub fn player_movement(mut player: Query<(&mut Player, &mut Transform)>, time: Res<Time>) {
    for (mut player, mut transform) in player.iter() {
        let mut translation = transform.get_translation();

        translation += player.velocity * time.delta_seconds;

        transform.set_translation(translation);
    }
}
