use std::{future::Future, rc::Rc};

use genawaiter::{rc::gen, yield_};

use crate::mesh::Mesh;

#[derive(Clone, Debug)]
pub enum Object {
    Mesh(Rc<Mesh>),
    Node(Rc<Node>),
}

#[derive(Debug)]
pub struct Node {
    children: Vec<Object>,
}

impl Node {
    pub fn new() -> Self {
        Self { children: vec![] }
    }

    pub fn add_child(mut self, child: Object) -> Self {
        self.children.push(child);

        self
    }

    pub fn depth_first<'a>(
        &'a self,
    ) -> genawaiter::rc::Gen<&'a Object, (), impl Future<Output = ()> + use<'a>> {
        gen!({
            let mut stack: Vec<&'a Node> = vec![self];

            loop {
                match stack.pop() {
                    Some(node) => {
                        for child in &node.children {
                            match child {
                                Object::Node(n) => {
                                    stack.push(n);
                                }
                                x => yield_!(x),
                            }
                        }
                    }
                    None => break,
                }
            }
        })
    }
}
