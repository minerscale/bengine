use std::{future::Future, rc::Rc};

use genawaiter::{rc::gen, yield_};

use crate::mesh::Mesh;

use ultraviolet::Isometry3;

#[derive(Clone, Debug)]
pub enum Object {
    Mesh(Rc<Mesh>),
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
            transform: transform,
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

    pub fn breadth_first<'a>(
        &'a self,
    ) -> genawaiter::rc::Gen<(Isometry3, &'a Node), (), impl Future<Output = ()> + use<'a>> {
        gen!({
            let mut stack: Vec<(Isometry3, &'a Node)> = vec![(self.transform, self)];

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
