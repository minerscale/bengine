use std::{future::Future, rc::Rc};

use genawaiter::{rc::gen, yield_};

use crate::{collision::Polyhedron, mesh::Mesh, texture::Texture};

use ultraviolet::{Isometry3, Vec3};

#[derive(Clone, Debug)]
pub enum Object {
    Model((Rc<Mesh>, Rc<Texture>)),
    Collider(Polyhedron<Vec3>),
}

#[derive(Debug)]
pub struct Node {
    pub transform: Isometry3,
    pub children: Vec<Node>,
    pub objects: Vec<Object>,
}

impl Node {
    pub fn empty() -> Self {
        Self {
            transform: Isometry3::identity(),
            children: vec![],
            objects: vec![],
        }
    }

    pub fn new(transform: Isometry3, children: Vec<Node>, objects: Vec<Object>) -> Self {
        Self {
            transform,
            children,
            objects,
        }
    }

    pub fn add_child(mut self, child: Node) -> Self {
        self.children.push(child);

        self
    }

    pub fn add_object(mut self, object: Object) -> Self {
        self.objects.push(object);

        self
    }

    pub fn breadth_first(
        &self,
    ) -> genawaiter::rc::Gen<(Isometry3, &Node), (), impl Future<Output = ()> + use<'_>> {
        gen!({
            let mut stack: Vec<(Isometry3, &Node)> = vec![(self.transform, self)];

            loop {
                match stack.pop() {
                    Some((transform, node)) => {
                        for child in &node.children {
                            let t = transform * child.transform;

                            if !self.children.is_empty() {
                                stack.push((t, child));
                            }

                            yield_!((t, child));
                        }
                    }
                    None => break,
                }
            }
        })
    }
}
