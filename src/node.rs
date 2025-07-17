use tracing_mutex::stdsync::Mutex;

use std::{future::Future, sync::Arc};

use genawaiter::{rc::r#gen, yield_};
use rapier3d::prelude::{Collider, ColliderHandle, RigidBody, RigidBodyHandle};

use crate::{mesh::Mesh, physics::Physics, player::Player};

use ultraviolet::Isometry3;

#[derive(Debug)]
pub enum Object {
    Mesh(Arc<Mesh>),
    Collider(ColliderHandle),
    RigidBody((ColliderHandle, RigidBodyHandle)),
    Player(Player),
}

pub struct Node {
    transform: Isometry3,
    previous_transform: Isometry3,
    pub children: Vec<Arc<Mutex<Node>>>,
    pub objects: Vec<Object>,
}

#[derive(Clone)]
pub struct GameTree {
    pub root_node: Arc<Mutex<Node>>,
}

impl GameTree {
    pub fn new<T: Into<Arc<Mutex<Node>>>>(root_node: T) -> Self {
        Self {
            root_node: root_node.into(),
        }
    }

    pub fn breadth_first(
        &self,
    ) -> genawaiter::rc::Gen<
        (Isometry3, Isometry3, Arc<Mutex<Node>>),
        (),
        impl Future<Output = ()> + use<'_>,
    > {
        r#gen!({
            let root_node = self.root_node.lock().unwrap();

            let mut stack: Vec<(Isometry3, Isometry3, Arc<Mutex<Node>>)> = vec![(
                root_node.previous_transform,
                root_node.transform,
                self.root_node.clone(),
            )];

            drop(root_node);

            loop {
                match stack.pop() {
                    Some((previous_transform, transform, node)) => {
                        for child in &node.lock().unwrap().children {
                            let child_lock = child.lock().unwrap();

                            let previous_t = previous_transform * child_lock.previous_transform;
                            let t = transform * child_lock.transform;

                            drop(child_lock);

                            stack.push((previous_t, t, child.clone()));
                        }
                        yield_!((previous_transform, transform, node));
                    }
                    None => break,
                }
            }
        })
    }
}

impl From<Node> for Arc<Mutex<Node>> {
    fn from(value: Node) -> Self {
        Self::new(Mutex::new(value))
    }
}

impl Node {
    pub fn empty() -> Self {
        Self {
            transform: Isometry3::identity(),
            previous_transform: Isometry3::identity(),
            children: vec![],
            objects: vec![],
        }
    }

    pub fn set_transform(&mut self, transform: Isometry3) {
        self.previous_transform = self.transform;
        self.transform = transform;
    }

    pub fn transform(&self) -> Isometry3 {
        self.transform
    }

    pub fn previous_transform(&self) -> Isometry3 {
        self.previous_transform
    }

    pub fn new(
        transform: Isometry3,
        children: Vec<Arc<Mutex<Self>>>,
        objects: Vec<Object>,
    ) -> Self {
        Self {
            transform,
            previous_transform: transform,
            children,
            objects,
        }
    }

    pub fn child<T: Into<Arc<Mutex<Self>>>>(mut self, child: T) -> Self {
        self.children.push(child.into());

        self
    }

    pub fn mesh(mut self, mesh: Arc<Mesh>) -> Self {
        self.objects.push(Object::Mesh(mesh));

        self
    }

    pub fn player(mut self, player: Player) -> Self {
        self.objects.push(Object::Player(player));

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
