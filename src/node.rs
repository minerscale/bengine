use std::{cell::RefCell, future::Future, rc::Rc};

use genawaiter::{rc::r#gen, yield_};

use crate::{mesh::Mesh, renderer::texture::Texture};

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
        Self { root_node }
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

    pub fn add_child(mut self, child: Rc<RefCell<Self>>) -> Self {
        self.children.push(child);

        self
    }

    pub fn add_object(mut self, object: Object) -> Self {
        self.objects.push(object);

        self
    }
}
