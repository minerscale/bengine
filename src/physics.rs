use rapier3d::{
    math::Vector,
    na::vector,
    prelude::{
        CCDSolver, ColliderSet, DefaultBroadPhase, ImpulseJointSet, IntegrationParameters,
        IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline, QueryPipeline, Real,
        RigidBodySet,
    },
};
use ultraviolet::{Isometry3, Rotor3, Vec3};

use crate::{
    node::{Node, Object},
    player::Player,
};

pub struct Physics {
    pub gravity: Vector<Real>,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub query_pipeline: QueryPipeline,
    pub physics_hooks: (),
    pub event_handler: (),
}

impl Default for Physics {
    fn default() -> Self {
        Self::new()
    }
}

impl Physics {
    pub fn new() -> Self {
        Self {
            gravity: vector![0.0, -9.81, 0.0],
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            physics_hooks: (),
            event_handler: (),
        }
    }

    pub fn step(&mut self, scene: &mut [Node], player: &mut Player, dt: f32) {
        self.integration_parameters.dt = dt;

        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &self.physics_hooks,
            &self.event_handler,
        );

        for node in scene.iter_mut() {
            let transform = node.objects.iter().find_map(|o| match o {
                Object::RigidBody((_, rigid_body_handle)) => Some(from_nalgebra(
                    self.rigid_body_set[*rigid_body_handle].position(),
                )),

                _ => None,
            });

            if let Some(transform) = transform {
                node.set_transform(transform);
            }
        }

        player.previous_position = player.position;
        player.position =
            from_nalgebra(self.rigid_body_set[player.rigid_body_handle].position()).translation;
    }
}

pub fn from_nalgebra(p: &rapier3d::na::Isometry3<f32>) -> Isometry3 {
    Isometry3::new(
        Vec3::from(p.translation.vector.as_slice().first_chunk().unwrap()),
        Rotor3::from_quaternion_array(*p.rotation.coords.as_slice().first_chunk().unwrap()),
    )
}
