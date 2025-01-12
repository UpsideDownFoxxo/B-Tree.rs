use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;

use super::node::{Node, NodeIdent};

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
    fn get_node(&mut self, ident: NodeIdent) -> Result<&mut Node<T, S>, NodeStoreError>;
    fn store_node(&mut self, node: Node<T, S>, is_leaf: bool) -> Result<NodeIdent, NodeStoreError>;
    fn print_stored_nodes(&mut self, root: NodeIdent) -> ();
    fn flush(&mut self) -> ();
}

pub type SharedNodeStore<T, const S: usize> = Rc<RefCell<dyn NodeStore<T, S>>>;

impl<const S: usize, T> NodeStore<T, S> for (HashMap<NodeIdent, Node<T, S>>, NodeIdent)
where
    T: Sized,
    T: Debug,
{
    fn get_node(&mut self, ident: NodeIdent) -> Result<&mut Node<T, S>, NodeStoreError> {
        let (nodes, _) = self;
        let node = nodes.get_mut(&ident);
        return match node {
            None => Err(NodeStoreError::InvalidReference),
            Some(n) => Ok(n),
        };
    }

    fn store_node(&mut self, node: Node<T, S>, is_leaf: bool) -> Result<NodeIdent, NodeStoreError> {
        let (nodes, _) = self;

        let key = if is_leaf {
            self.1 += 1;
            self.1
        } else {
            self.1 += 1;
            -self.1
        };

        nodes.insert(key, node);

        println!("Created node {key}");
        return Ok(key);
    }

    fn print_stored_nodes(&mut self, _root: NodeIdent) -> () {
        let (nodes, _) = self;
        nodes
            .iter()
            .for_each(|e| println!("{}", e.1.to_graphviz(&e.0.clone())));
    }

    // this does nothing since this version of the node store is entirely in memory
    fn flush(&mut self) -> () {
        ()
    }
}
