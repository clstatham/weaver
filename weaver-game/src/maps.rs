use weaver_core::{material::Material, mesh::Mesh, transform::Transform};
use weaver_proc_macro::{Bundle, Component};

#[derive(Debug, Clone, Copy, Component)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ground;

#[derive(Bundle)]
pub struct GroundBundle {
    pub ground: Ground,
    pub transform: Transform,
    pub mesh: Mesh,
    pub material: Material,
}
