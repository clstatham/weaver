use weaver_proc_macro::Bundle;

use super::{material::Material, mesh::Mesh, transform::Transform};

#[derive(Bundle)]
pub struct Model {
    pub mesh: Mesh,
    pub transform: Transform,
    pub material: Material,
}
