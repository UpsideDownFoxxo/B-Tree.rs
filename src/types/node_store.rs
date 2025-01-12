use crate::types::node::{InnerNode, LeafNode, NodeIdent, NodeInstance, NodeRef};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;

use super::node::Node;

#[derive(Debug)]
pub enum NodeStoreError {
    InvalidReference,
    WriteFailed,
    ReadFailed,
}

pub trait NodeStore<T, const S: usize>
where
    T: Sized,
    T: Debug,
{
    fn get_node(&mut self, ident: NodeIdent) -> Result<NodeRef<T, S>, NodeStoreError>;
    fn store_node(&mut self, node: NodeInstance<T, S>) -> Result<NodeIdent, NodeStoreError>;
    fn print_stored_nodes(&mut self, root: NodeIdent) -> ();
}

pub type SharedNodeStore<T, const S: usize> = Rc<RefCell<dyn NodeStore<T, S>>>;

impl<const S: usize, T> NodeStore<T, S>
    for (
        HashMap<NodeIdent, InnerNode<T, S>>,
        HashMap<NodeIdent, LeafNode<T, S>>,
        NodeIdent,
        NodeIdent,
    )
where
    T: Sized,
    T: Debug,
{
    fn get_node(&mut self, ident: NodeIdent) -> Result<NodeRef<T, S>, NodeStoreError> {
        let (inner, leaves, _, _) = self;
        if ident < 0 {
            let node = inner.get_mut(&-ident);
            return match node {
                None => Err(NodeStoreError::InvalidReference),
                Some(n) => Ok(NodeRef::Inner(n)),
            };
        }

        let node = leaves.get_mut(&ident);
        return match node {
            None => Err(NodeStoreError::InvalidReference),
            Some(n) => Ok(NodeRef::Leaf(n)),
        };
    }

    fn store_node(&mut self, node: NodeInstance<T, S>) -> Result<NodeIdent, NodeStoreError> {
        let (inner, leaves, _, _) = self;

        match node {
            NodeInstance::Inner(n) => {
                self.2 += 1;
                inner.insert(self.2, n);
                return Ok(-self.2);
            }
            NodeInstance::Leaf(l) => {
                self.3 += 1;
                leaves.insert(self.3, l);

                return Ok(self.3);
            }
        };
    }

    fn print_stored_nodes(&mut self, _root: NodeIdent) -> () {
        let (inner, leaves, _, _) = self;
        inner
            .iter()
            .for_each(|e| println!("{}", e.1.to_graphviz(&-e.0.clone())));
        leaves
            .iter()
            .for_each(|e| println!("{}", e.1.to_graphviz(e.0)));
    }
}
