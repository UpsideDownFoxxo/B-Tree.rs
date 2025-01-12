#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use std::{cell::RefCell, collections::HashMap, marker::PhantomData, rc::Rc};

use random::Source;
use types::{
    file_writer::{FileStore, BLOCK_SIZE},
    node::{Data, InnerNode, InsertionResult, LeafNode, Node, NodeIdent, NodeInstance, SearchKey},
    node_store::SharedNodeStore,
};

pub mod types;

const SIZE_PER_ENTRY: usize = size_of::<SearchKey>() + size_of::<NodeIdent>();
const FANOUT: usize = (BLOCK_SIZE / SIZE_PER_ENTRY) / 2 * 2;

fn main() {
    let node_store: (
        HashMap<NodeIdent, InnerNode<i64, FANOUT>>,
        HashMap<NodeIdent, LeafNode<i64, FANOUT>>,
        NodeIdent,
        NodeIdent,
    ) = (HashMap::new(), HashMap::new(), 0, 0);

    let node_store = FileStore::new("test_tree".to_string()).unwrap();

    let shared_node_store: SharedNodeStore<i64, FANOUT> = Rc::new(RefCell::new(node_store));

    let leaf: LeafNode<i64, FANOUT> = LeafNode {
        keys: [0; FANOUT],
        data_blocks: [0; FANOUT],
        size: 0,
        phantom: std::marker::PhantomData,
    };

    let mut root_ident = {
        let mut node_store = shared_node_store.borrow_mut();
        node_store
            .store_node(NodeInstance::Leaf(leaf))
            .expect("Unable to store node")
    };

    let mut insert = random::default(0);

    for _i in 1..200 {
        let res = if root_ident > 0 {
            LeafNode::insert(
                root_ident,
                (insert.read_u64() as i8).into(),
                Data { data: 64 },
                shared_node_store.clone(),
            )
        } else {
            InnerNode::insert(
                root_ident,
                (insert.read_u64() as i8).into(),
                Data { data: 64 },
                shared_node_store.clone(),
            )
        };

        match res {
            InsertionResult::Ok => (),
            InsertionResult::NodeOverflow(separator, ident, _) => {
                let mut separators = [0; FANOUT];
                let mut children = [0; FANOUT];

                separators[0] = separator;
                children[0] = root_ident;
                children[1] = ident;

                let new_root = InnerNode {
                    size: 1,
                    separators,
                    children,
                    phantom: PhantomData,
                };

                let mut node_store = shared_node_store.borrow_mut();
                let root_ident_new = node_store
                    .store_node(NodeInstance::Inner(new_root))
                    .unwrap();
                root_ident = root_ident_new;
            }
            InsertionResult::DuplicateKey => println!("Tried to insert duplicate key"),

            e => {
                println!("Unable to insert: {e:?}");
                panic!("Bye");
            }
        }
    }

    let mut node_store = shared_node_store.borrow_mut();
    node_store.print_stored_nodes(root_ident);
}
