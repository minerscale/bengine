use std::{
    cell::{Cell, RefCell},
    future::Future,
    rc::Rc,
};

use genawaiter::{rc::gen, yield_};

use crate::{
    collision::Polyhedron,
    physics::RigidBody,
    renderer::{mesh::Mesh, texture::Texture},
};

use ultraviolet::{Isometry3, Vec3};

#[derive(Clone, Debug)]
pub enum Object {
    Model((Rc<Mesh>, Rc<Texture>)),
    Collider(Polyhedron<Vec3>),
    RigidBody(RigidBody),
}

pub struct Node {
    pub transform: Cell<Isometry3>,
    pub children: Vec<Node>,
    pub objects: RefCell<Vec<Object>>,
}

impl Node {
    pub fn empty() -> Self {
        Self {
            transform: Isometry3::identity().into(),
            children: vec![],
            objects: vec![].into(),
        }
    }

    pub fn new(transform: Isometry3, children: Vec<Node>, objects: Vec<Object>) -> Self {
        Self {
            transform: transform.into(),
            children,
            objects: objects.into(),
        }
    }

    pub fn add_child(mut self, child: Node) -> Self {
        self.children.push(child);

        self
    }

    pub fn add_object(mut self, object: Object) -> Self {
        self.objects.get_mut().push(object);

        self
    }

    pub fn breadth_first(
        &self,
    ) -> genawaiter::rc::Gen<(Isometry3, &Node), (), impl Future<Output = ()> + use<'_>> {
        gen!({
            let mut stack: Vec<(Isometry3, &Node)> = vec![(self.transform.get(), self)];

            loop {
                match stack.pop() {
                    Some((transform, node)) => {
                        for child in &node.children {
                            let t = transform * child.transform.get();

                            stack.push((t, child));
                        }
                        yield_!((transform, node));
                    }
                    None => break,
                }
            }
        })
    }
}
