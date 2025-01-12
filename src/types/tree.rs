use crate::types::{node::NodeIdent, node_store::NodeStore};

struct Tree<T: Sized, const S: usize> {
    store: Box<dyn NodeStore<T, S>>,
    root: NodeIdent,
}

impl<T: Sized, const S: usize> Tree<T, S> {}
