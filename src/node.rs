use std::sync::Arc;

use rapier3d::prelude::{Collider, ColliderHandle, RigidBody, RigidBodyHandle};
use ultraviolet::Isometry3;

use crate::{mesh::Mesh, physics::Physics};

#[derive(Debug)]
pub enum Object {
    Mesh(Arc<Mesh>),
    RigidBody((ColliderHandle, RigidBodyHandle)),
    #[allow(dead_code)]
    Collider(ColliderHandle),
}

pub struct Node {
    pub transform: Isometry3,
    pub previous_transform: Isometry3,
    pub objects: Vec<Object>,
}

impl Node {
    pub fn empty() -> Self {
        Self {
            transform: Isometry3::identity(),
            previous_transform: Isometry3::identity(),
            objects: vec![],
        }
    }

    pub fn set_transform(&mut self, transform: Isometry3) {
        self.previous_transform = self.transform;
        self.transform = transform;
    }

    pub fn mesh(mut self, mesh: Arc<Mesh>) -> Self {
        self.objects.push(Object::Mesh(mesh));

        self
    }

    pub fn collider<T: Into<Collider>>(mut self, physics: &mut Physics, collider: T) -> Self {
        let collider = physics.collider_set.insert(collider);

        self.objects.push(Object::Collider(collider));

        self
    }

    pub fn rigid_body<T: Into<Collider>, U: Into<RigidBody>>(
        mut self,
        physics: &mut Physics,
        collider: T,
        rigid_body: U,
    ) -> Self {
        let rigid_body = physics.rigid_body_set.insert(rigid_body);
        let collider = physics.collider_set.insert_with_parent(
            collider,
            rigid_body,
            &mut physics.rigid_body_set,
        );

        self.objects.push(Object::RigidBody((collider, rigid_body)));

        self
    }
}
