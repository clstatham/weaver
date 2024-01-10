use crate::prelude::*;

#[derive(Debug, Clone, Copy, Component)]
pub struct Player {
    pub speed: f32,
    pub rotation_speed: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            speed: 10.0,
            rotation_speed: 1.0,
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

#[system(PlayerUpdate)]
pub fn player_update(
    mut player: Query<(&Player, &mut Transform)>,
    input: Res<Input>,
    time: Res<Time>,
) {
    for (player, mut transform) in player.iter() {
        let mut translation = transform.get_translation();
        let mut rotation = transform.get_rotation();

        let mouse_delta = input.mouse_delta();

        if input.mouse_button_pressed(3) {
            let delta = mouse_delta * player.rotation_speed;
            rotation = Quat::from_rotation_y(-delta.x) * rotation;
        }

        let mut direction = Vec3::ZERO;

        if input.key_pressed(KeyCode::W) {
            direction += rotation * Vec3::Z;
        }
        if input.key_pressed(KeyCode::S) {
            direction -= rotation * Vec3::Z;
        }
        if input.key_pressed(KeyCode::A) {
            direction += rotation * Vec3::X;
        }
        if input.key_pressed(KeyCode::D) {
            direction -= rotation * Vec3::X;
        }
        if input.key_pressed(KeyCode::Space) {
            direction += rotation * Vec3::Y;
        }
        if input.key_pressed(KeyCode::LControl) {
            direction -= rotation * Vec3::Y;
        }

        translation += direction.normalize_or_zero() * player.speed * time.delta_seconds;

        transform.set_translation(translation);
        transform.set_rotation(rotation);
    }
}
