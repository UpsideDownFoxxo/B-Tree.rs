use crate::types::node_store::{NodeStore, NodeStoreError, SharedNodeStore};
use std::{cell::RefCell, cmp, fmt::Debug, marker::PhantomData, rc::Rc};

pub type NodeIdent = i32;
pub type SearchKey = i64;

// We use generics here because rust normally doesn't allow usage of variable length arrays in structs
// the phantom field will be removed during compilation, it's just here so we can correctly infer
// leaf types
// https://doc.rust-lang.org/nomicon/phantom-data.html
#[derive(Debug)]
pub struct Node<T, const S: usize>
where
    T: Sized,
    T: Debug,
{
    // separators is only populated up to S-1
    pub separators: [SearchKey; S],
    pub children: [NodeIdent; S],
    pub size: usize,
    pub phantom: PhantomData<T>,
}

pub struct Data<T: Sized> {
    pub data: T,
}

pub enum NodeCreationError {
    CapacityExceeded,
}

#[derive(Debug)]
pub enum InsertionResult<T, const S: usize> {
    Ok,
    NodeOverflow(SearchKey, NodeIdent, PhantomData<T>),
    Error(String),
    InsertError(NodeStoreError),
    DuplicateKey,
}

/// inserts the given key into the array, moving all following elements accordingly
fn insert_into_array<T>(slice: &mut [T], index: usize, key: T, empty: T) -> Option<T>
where
    T: cmp::PartialEq,
    T: Copy,
    T: Debug,
{
    let len = slice.len();
    let mut hanging = key;

    // continue swapping keys until end of the slice is reached.
    for i in index..len {
        let tmp = slice[i];
        slice[i] = hanging;

        // return early if we read a null value, since that means the array wasn't full
        if tmp == empty {
            return None;
        }

        hanging = tmp;
    }

    Some(hanging)
}

impl<T, const S: usize> Node<T, S>
where
    T: Sized,
    T: Debug,
{
    pub fn insert(
        self_id: NodeIdent,
        key: SearchKey,
        data: NodeIdent,
        shared_node_store: SharedNodeStore<T, S>,
    ) -> InsertionResult<T, S> {
        if self_id < 0 {
            Node::insert_inner(self_id, key, data, shared_node_store)
        } else {
            Node::insert_leaf(self_id, key, data, shared_node_store)
        }
    }

    pub fn search(
        self_id: NodeIdent,
        key: SearchKey,
        shared_node_store: SharedNodeStore<T, S>,
    ) -> Result<Option<NodeIdent>, NodeStoreError> {
        let child = {
            let mut node_store = shared_node_store.borrow_mut();
            let node = node_store.get_node(self_id)?;

            let separators = &node.separators[0..node.size];

            let subtree_index = match separators.binary_search(&key) {
                Ok(r) => {
                    if self_id < 0 {
                        r
                    } else {
                        return Ok(Some(node.children[r]));
                    }
                }
                Err(r) => {
                    if self_id < 0 {
                        r
                    } else {
                        return Ok(None);
                    }
                }
            };

            node.children[subtree_index]
        };

        return Node::search(child, key, shared_node_store);
    }

    fn insert_inner(
        self_id: NodeIdent,
        key: SearchKey,
        data: NodeIdent,
        shared_node_store: SharedNodeStore<T, S>,
    ) -> InsertionResult<T, S> {
        let (insert_child, insertion_index) = {
            let mut node_store = shared_node_store.borrow_mut();
            let current_node = match node_store.get_node(self_id) {
                Ok(n) => n,
                Err(e) => return InsertionResult::InsertError(e),
            };

            let insertion_index =
                match &current_node.separators[0..current_node.size].binary_search(&key) {
                    Ok(_) => return InsertionResult::DuplicateKey,
                    Err(u) => u,
                }
                .clone();

            (current_node.children[insertion_index], insertion_index)
        };

        // we explicitly drop our node_store by exiting the scope so the child node can open the refcell without panic
        let res = Node::insert(insert_child, key, data, shared_node_store.clone());

        let (new_sep, new_node_ident) = match res {
            InsertionResult::NodeOverflow(new_sep, new_node_ident, _phantom) => {
                (new_sep, new_node_ident)
            }
            r => return r,
        };

        // we need to fix up the current node, but we dropped our previous reference.
        let mut node_store = shared_node_store.borrow_mut();
        let current_node = match node_store.get_node(self_id) {
            Ok(n) => n,
            Err(e) => return InsertionResult::InsertError(e),
        };

        // Since we access the separators on a smaller slice, the two will overflow at the same time
        let overflow_key = insert_into_array::<SearchKey>(
            &mut current_node.separators[0..S - 1],
            insertion_index,
            new_sep,
            0,
        );

        // our returned node is a right subtree to ident, so it has to be inserted one to the right
        let overflow_value = insert_into_array::<NodeIdent>(
            &mut current_node.children[0..S],
            insertion_index + 1,
            new_node_ident,
            0,
        );

        current_node.size += 1;

        let (key, value) = match (overflow_key, overflow_value) {
            (None, None) => return InsertionResult::Ok,
            (Some(key), Some(value)) => (key, value),
            (key, value) => {
                return InsertionResult::Error(format!(
                    "Mismatched overflow: key was {key:?}, value was {value:?}"
                ))
            }
        };

        let (root_sep, right_seps, right_children) = current_node.split_inner(key, value);

        let right_node = Node {
            children: right_children,
            separators: right_seps,
            size: S / 2,
            phantom: PhantomData::<T>,
        };

        let right_node_ident = match node_store.store_node(right_node, false) {
            Ok(i) => i,
            Err(_) => todo!(),
        };

        InsertionResult::NodeOverflow(root_sep, right_node_ident, PhantomData::<T>)
    }

    fn split_inner(
        &mut self,
        largest_key: SearchKey,
        largest_value: NodeIdent,
    ) -> (SearchKey, [SearchKey; S], [NodeIdent; S]) {
        let target_size = S / 2;

        // Seps is actually populated to size S-1, which means the right slice has a size of target_size
        let right_seps_slice = &mut self.separators[target_size - 1..S - 1];
        let right_children_slice = &mut self.children[target_size..S];

        let mut right_seps = [0; S];
        let mut right_children = [0; S];

        let root_sep = right_seps_slice[0];

        // do not copy the first key, since that is the new separator
        right_seps[0..target_size - 1].copy_from_slice(&right_seps_slice[1..target_size]);
        right_children[0..target_size].copy_from_slice(right_children_slice);

        // insert elements that did not fit into the original node
        right_seps[target_size - 1] = largest_key;
        right_children[target_size] = largest_value;

        // update left node
        right_seps_slice.fill(0);
        right_children_slice.fill(0);
        self.size = target_size - 1;

        (root_sep, right_seps, right_children)
    }

    fn insert_leaf(
        self_id: NodeIdent,
        key: SearchKey,
        data: NodeIdent,
        shared_node_store: Rc<RefCell<(dyn NodeStore<T, S>)>>,
    ) -> InsertionResult<T, S> {
        let mut node_store = shared_node_store.borrow_mut();
        let current_node = match node_store.get_node(self_id) {
            Ok(n) => n,

            Err(e) => return InsertionResult::InsertError(e),
        };

        let separators = &current_node.separators[0..current_node.size];

        let insertion_index = match separators.binary_search(&key) {
            Ok(_u) => return InsertionResult::DuplicateKey,
            Err(u) => u,
        };

        let overflow_key = insert_into_array::<SearchKey>(
            &mut current_node.separators[0..S],
            insertion_index,
            key,
            0,
        );

        let overflow_value = insert_into_array::<NodeIdent>(
            &mut current_node.children[0..S],
            insertion_index,
            data,
            0,
        );

        current_node.size += 1;

        let (key, value) = match (overflow_key, overflow_value) {
            (None, None) => return InsertionResult::Ok,
            (Some(key), Some(value)) => (key, value),
            (key, value) => {
                return InsertionResult::Error(format!(
                    "Mismatched overflow: key was {key:?}, value was {value:?}"
                ))
            }
        };

        let (root_sep, right_seps, right_children) = current_node.split_leaf(key, value);

        let right_node = Node {
            children: right_children,
            separators: right_seps,
            size: S / 2 + 1,
            phantom: PhantomData::<T>,
        };

        let right_node_ident = match node_store.store_node(right_node, true) {
            Ok(i) => i,
            Err(_) => panic!("Unable to store newly created node"),
        };

        InsertionResult::NodeOverflow(root_sep, right_node_ident, PhantomData::<T>)
    }

    fn split_leaf(
        &mut self,
        largest_key: SearchKey,
        largest_value: NodeIdent,
    ) -> (SearchKey, [SearchKey; S], [NodeIdent; S]) {
        let target_size = S / 2;

        let right_seps_slice = &mut self.separators[target_size..S];
        let right_children_slice = &mut self.children[target_size..S];

        let mut right_seps = [0; S];
        let mut right_children = [0; S];

        // smallest key of the left node becomes the new separator
        let root_sep = right_seps_slice[0];

        right_seps[0..target_size].copy_from_slice(right_seps_slice);
        right_children[0..target_size].copy_from_slice(right_children_slice);

        // insert elements that did not fit into the original node
        right_seps[target_size] = largest_key;
        right_children[target_size] = largest_value;

        // update left node
        right_seps_slice.fill(0);
        right_children_slice.fill(0);
        self.size = target_size;

        (root_sep, right_seps, right_children)
    }

    pub fn to_graphviz(&self, node_id: &NodeIdent) -> String {
        if *node_id < 0 {
            let mut result = format!("{} [shape=record,label=\"<sep0> ", node_id);
            for i in 0..self.size {
                result.push_str(&format!("| {} | <sep{}> ", self.separators[i], i + 1));
            }
            result.push_str("\"];");

            for i in 0..=self.size {
                result.push_str(&format!("\n{}:sep{} -> {};", node_id, i, self.children[i]));
            }

            result
        } else {
            let mut result = format!("{} [shape=record, label=\"", node_id);
            for i in 0..self.size {
                result.push_str(&format!("{{ {} }}", self.separators[i]));
                if i < self.size - 1 {
                    result.push_str(" | ");
                }
            }
            result.push_str("\"];");
            result
        }
    }
}
