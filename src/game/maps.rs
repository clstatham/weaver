use crate::prelude::*;

#[derive(Debug, Clone, Copy, Component)]
pub struct Ground;

#[derive(Bundle)]
pub struct GroundBundle {
    pub ground: Ground,
    pub transform: Transform,
    pub mesh: Mesh,
    pub material: Material,
}
