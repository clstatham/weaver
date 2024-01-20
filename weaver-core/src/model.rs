use weaver_ecs::prelude::*;

use super::{material::Material, mesh::Mesh, physics::RigidBody, transform::Transform};

#[derive(Bundle)]
pub struct ModelBundle {
    pub mesh: Mesh,
    pub transform: Transform,
    pub material: Material,
}

#[derive(Bundle)]
pub struct RigidBodyModelBundle {
    pub mesh: Mesh,
    pub material: Material,
    pub rigid_body: RigidBody,
}
