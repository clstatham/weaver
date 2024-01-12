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
