use std::{cell::RefCell, future::Future, rc::Rc};

use genawaiter::{rc::r#gen, yield_};
use rapier3d::prelude::{Collider, ColliderHandle, RigidBody, RigidBodyHandle};

use crate::{mesh::Mesh, physics::Physics, renderer::texture::Texture};

use ultraviolet::Isometry3;

#[derive(Clone, Debug)]
pub enum Object {
    Model((Rc<Mesh>, Rc<Texture>)),
    Collider(ColliderHandle),
    RigidBody((ColliderHandle, RigidBodyHandle)),
}

pub struct Node {
    pub transform: Isometry3,
    pub children: Vec<Rc<RefCell<Node>>>,
    pub objects: Vec<Object>,
}

pub struct GameTree {
    pub root_node: Rc<RefCell<Node>>,
}

impl GameTree {
    pub fn new<T: Into<Rc<RefCell<Node>>>>(root_node: T) -> Self {
        Self {
            root_node: root_node.into(),
        }
    }

    pub fn breadth_first(
        &self,
    ) -> genawaiter::rc::Gen<(Isometry3, Rc<RefCell<Node>>), (), impl Future<Output = ()> + use<'_>>
    {
        r#gen!({
            let mut stack: Vec<(Isometry3, Rc<RefCell<Node>>)> =
                vec![(self.root_node.borrow().transform, self.root_node.clone())];

            loop {
                match stack.pop() {
                    Some((transform, node)) => {
                        for child in &node.borrow().children {
                            let t = transform * child.borrow().transform;

                            stack.push((t, child.clone()));
                        }
                        yield_!((transform, node));
                    }
                    None => break,
                }
            }
        })
    }
}

impl From<Node> for Rc<RefCell<Node>> {
    fn from(value: Node) -> Self {
        Self::new(RefCell::new(value))
    }
}

impl Node {
    pub fn empty() -> Self {
        Self {
            transform: Isometry3::identity(),
            children: vec![],
            objects: vec![],
        }
    }

    pub fn new(
        transform: Isometry3,
        children: Vec<Rc<RefCell<Self>>>,
        objects: Vec<Object>,
    ) -> Self {
        Self {
            transform,
            children,
            objects,
        }
    }

    pub fn child<T: Into<Rc<RefCell<Self>>>>(mut self, child: T) -> Self {
        self.children.push(child.into());

        self
    }

    pub fn model(mut self, mesh: Rc<Mesh>, texture: Rc<Texture>) -> Self {
        self.objects.push(Object::Model((mesh, texture)));

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
