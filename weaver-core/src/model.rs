use weaver_ecs::prelude::*;

use crate::transform::Transform;

use super::{material::Material, mesh::Mesh, physics::RigidBody, transform::GlobalTransform};

#[derive(Bundle)]
pub struct ModelBundle {
    pub mesh: Mesh,
    pub global_transform: GlobalTransform,
    pub transform: Transform,
    pub material: Material,
}

#[derive(Bundle)]
pub struct RigidBodyModelBundle {
    pub mesh: Mesh,
    pub material: Material,
    pub rigid_body: RigidBody,
}
