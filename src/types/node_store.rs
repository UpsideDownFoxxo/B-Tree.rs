use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

use super::{
    file_store::Metadata,
    node::{Node, NodeIdent},
};

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
    fn set_metadata(&mut self, data: Metadata);
    fn node_ctr(&self) -> NodeIdent;
}

pub type SharedNodeStore<T, const S: usize> = Rc<RefCell<dyn NodeStore<T, S>>>;

/// size of the file blocks in bytes
pub const BLOCK_SIZE: usize = 128;

pub trait ByteSerialize {
    fn to_bytes(&self) -> [u8; BLOCK_SIZE];
    fn from_bytes(block: [u8; BLOCK_SIZE]) -> Self;
}
