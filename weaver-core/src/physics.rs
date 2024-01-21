use std::fmt::Debug;

use rapier3d::prelude::*;
use weaver_proc_macro::{Component, Resource};

use super::transform::Transform;

#[derive(Resource)]
pub struct RapierContext {
    pub gravity: Vector<f32>,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub broad_phase: BroadPhase,
    pub narrow_phase: NarrowPhase,
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    pub ccd_solver: CCDSolver,
    pub island_manager: IslandManager,
    pub query_pipeline: QueryPipeline,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
}

impl RapierContext {
    pub fn new(gravity: glam::Vec3) -> Self {
        let gravity = gravity.into();
        let integration_parameters = IntegrationParameters::default();
        let physics_pipeline = PhysicsPipeline::new();
        let broad_phase = BroadPhase::new();
        let narrow_phase = NarrowPhase::new();
        let bodies = RigidBodySet::new();
        let colliders = ColliderSet::new();
        let ccd_solver = CCDSolver::new();
        let island_manager = IslandManager::new();
        let query_pipeline = QueryPipeline::new();
        let impulse_joint_set = ImpulseJointSet::new();
        let multibody_joint_set = MultibodyJointSet::new();

        Self {
            gravity,
            integration_parameters,
            physics_pipeline,
            broad_phase,
            narrow_phase,
            bodies,
            colliders,
            ccd_solver,
            island_manager,
            query_pipeline,
            impulse_joint_set,
            multibody_joint_set,
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.integration_parameters.dt = dt;
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );
    }

    fn insert_collider_with_parent(
        &mut self,
        collider: Collider,
        parent: RigidBodyHandle,
    ) -> ColliderHandle {
        self.colliders
            .insert_with_parent(collider, parent, &mut self.bodies)
    }

    pub fn cast_ray(
        &self,
        ray: Ray,
        max_len: f32,
        query_filter: QueryFilter,
    ) -> Option<(ColliderHandle, f32)> {
        self.query_pipeline.cast_ray(
            &self.bodies,
            &self.colliders,
            &ray,
            max_len,
            true,
            query_filter,
        )
    }

    pub fn cast_ray_and_get_normal(
        &self,
        ray: Ray,
        max_len: f32,
        query_filter: QueryFilter,
    ) -> Option<(ColliderHandle, RayIntersection)> {
        self.query_pipeline.cast_ray_and_get_normal(
            &self.bodies,
            &self.colliders,
            &ray,
            max_len,
            true,
            query_filter,
        )
    }

    pub fn add_impulse_joint(
        &mut self,
        body1: RigidBodyHandle,
        body2: RigidBodyHandle,
        joint: impl Into<GenericJoint>,
    ) -> ImpulseJointHandle {
        self.impulse_joint_set
            .insert(body1, body2, joint.into(), true)
    }

    pub fn remove_impulse_joint(&mut self, handle: ImpulseJointHandle) {
        self.impulse_joint_set.remove(handle, true);
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct InitializedRigidBody {
    rb: RigidBodyHandle,
    collider: ColliderHandle,
    scale: glam::Vec3,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum RigidBodyPhysics {
    Uninitialized {
        rb: Box<rapier3d::dynamics::RigidBody>,
        collider: Box<rapier3d::geometry::Collider>,
        scale: glam::Vec3,
    },
    Initialized(InitializedRigidBody),
}

impl RigidBodyPhysics {
    fn lazy_init(&mut self, ctx: &mut RapierContext) -> &mut InitializedRigidBody {
        match self {
            RigidBodyPhysics::Uninitialized {
                rb,
                collider,
                scale,
            } => {
                let rb = ctx.bodies.insert(rb.as_ref().clone());
                let collider = ctx.insert_collider_with_parent(collider.as_ref().clone(), rb);
                *self = RigidBodyPhysics::Initialized(InitializedRigidBody {
                    rb,
                    collider,
                    scale: *scale,
                });
                match self {
                    RigidBodyPhysics::Initialized(body) => body,
                    _ => unreachable!(),
                }
            }
            RigidBodyPhysics::Initialized(body) => body,
        }
    }
}

#[derive(Component, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RigidBody {
    physics: RigidBodyPhysics,
}

impl Debug for RigidBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RigidBody").finish()
    }
}

impl RigidBody {
    pub fn new(
        rb: rapier3d::dynamics::RigidBody,
        collider: rapier3d::geometry::Collider,
        scale: glam::Vec3,
    ) -> Self {
        Self {
            physics: RigidBodyPhysics::Uninitialized {
                rb: Box::new(rb),
                collider: Box::new(collider),
                scale,
            },
        }
    }

    pub fn new_no_collisions(rb: rapier3d::dynamics::RigidBody, scale: glam::Vec3) -> Self {
        Self {
            physics: RigidBodyPhysics::Uninitialized {
                rb: Box::new(rb),
                collider: Box::new(
                    ColliderBuilder::ball(1.0)
                        .collision_groups(InteractionGroups::none())
                        .build(),
                ),
                scale,
            },
        }
    }

    pub fn get_transform(&mut self, ctx: &mut RapierContext) -> Transform {
        let body = self.physics.lazy_init(ctx);
        let rb = ctx.bodies.get(body.rb).unwrap();
        let translation = rb.position().translation.vector.into();
        let rotation = rb.position().rotation.into();
        let scale = body.scale;
        Transform::from_scale_rotation_translation(scale, rotation, translation)
    }

    pub fn add_force(&mut self, force: glam::Vec3, ctx: &mut RapierContext) {
        let body = self.physics.lazy_init(ctx);
        let rb = ctx.bodies.get_mut(body.rb).unwrap();
        rb.add_force(force.into(), true);
    }

    pub fn apply_impulse(&mut self, impulse: glam::Vec3, ctx: &mut RapierContext) {
        let body = self.physics.lazy_init(ctx);
        let rb = ctx.bodies.get_mut(body.rb).unwrap();
        rb.apply_impulse(impulse.into(), true);
    }

    pub fn collider_handle(&mut self, ctx: &mut RapierContext) -> ColliderHandle {
        let body = self.physics.lazy_init(ctx);
        body.collider
    }

    pub fn body_handle(&mut self, ctx: &mut RapierContext) -> RigidBodyHandle {
        let body = self.physics.lazy_init(ctx);
        body.rb
    }
}

// #[system(Physics)]
// pub fn physics(ctx: ResMut<RapierContext>, time: Res<Time>, bodies: Query<&mut RigidBody>) {
//     for mut body in bodies.iter() {
//         body.physics.lazy_init(&mut ctx);
//     }

//     ctx.step(time.delta_seconds);
// }
