use crate::prelude::*;

#[derive(Debug, Clone, Copy, Component)]
pub struct Npc {
    pub speed: f32,
    pub rotation_speed: f32,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            speed: 10.0,
            rotation_speed: 1.0,
        }
    }
}

#[derive(Bundle)]
pub struct NpcBundle {
    pub npc: Npc,
    pub transform: Transform,
    pub mesh: Mesh,
    pub material: Material,
}

#[system(NpcUpdate)]
pub fn npc_update(npc: Query<(&Npc, &mut Transform)>, time: Res<Time>) {
    for (npc, mut transform) in npc.iter() {
        let mut translation = transform.get_translation();
        let mut rotation = transform.get_rotation();

        let mut direction = Vec3::ZERO;

        // wander around in a circle for now
        rotation = Quat::from_rotation_y(-time.delta_seconds * npc.rotation_speed) * rotation;

        direction += rotation * Vec3::Z;

        translation += direction.normalize_or_zero() * npc.speed * time.delta_seconds;

        transform.set_translation(translation);
        transform.set_rotation(rotation);
    }
}
