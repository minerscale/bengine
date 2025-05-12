use std::{cell::RefCell, future::Future, rc::Rc};

use genawaiter::{rc::r#gen, yield_};

use crate::renderer::{mesh::Mesh, texture::Texture};

use ultraviolet::Isometry3;

#[derive(Clone, Debug)]
pub enum Object {
    Model((Rc<Mesh>, Rc<Texture>)),
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
    pub fn new(root_node: Rc<RefCell<Node>>) -> Self {
        GameTree { root_node }
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

impl Into<Rc<RefCell<Node>>> for Node {
    fn into(self) -> Rc<RefCell<Node>> {
        Rc::new(RefCell::new(self))
    }
}

impl Node {
    pub fn empty() -> Self {
        Self {
            transform: Isometry3::identity().into(),
            children: vec![],
            objects: vec![].into(),
        }
    }

    pub fn new(
        transform: Isometry3,
        children: Vec<Rc<RefCell<Node>>>,
        objects: Vec<Object>,
    ) -> Self {
        Self {
            transform,
            children,
            objects,
        }
    }

    pub fn add_child(mut self, child: Rc<RefCell<Node>>) -> Self {
        self.children.push(child);

        self
    }

    pub fn add_object(mut self, object: Object) -> Self {
        self.objects.push(object);

        self
    }
}
