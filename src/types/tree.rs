use std::{cell::RefCell, fmt::Debug, io, marker::PhantomData, rc::Rc};

use clap::error::Result;

use super::{
    file_store::{FileStore, LoadError, Metadata, BLOCK_SIZE},
    node::{Data, InsertionResult, Node, NodeIdent, SearchKey},
    node_store::{NodeStore, NodeStoreError},
};

pub struct Tree<T: Sized, const S: usize> {
    store: Rc<RefCell<dyn NodeStore<T, S>>>,
    root: NodeIdent,
}

pub enum TreeCreationError {
    IOError(io::Error),
    ParameterMismatch,
}

impl<T, const S: usize> Tree<T, S>
where
    T: Sized + 'static,

    T: Debug,
{
    pub fn insert(&mut self, key: SearchKey, value: NodeIdent) -> () {
        let res = Node::insert(self.root, key, value, self.store.clone());

        match res {
            InsertionResult::Ok => (),
            InsertionResult::NodeOverflow(separator, ident, _) => {
                let mut separators = [0; S];
                let mut children = [0; S];

                separators[0] = separator;
                children[0] = self.root;
                children[1] = ident;

                let new_root = Node {
                    size: 1,
                    separators,
                    children,
                    phantom: PhantomData,
                };

                let mut node_store = self.store.borrow_mut();
                let root_ident_new = node_store.store_node(new_root, false).unwrap();
                self.root = root_ident_new;
            }
            InsertionResult::DuplicateKey => println!("Tried to insert duplicate key"),

            e => {
                println!("Unable to insert: {e:?}");

                panic!("Bye");
            }
        }
    }

    pub fn save(&mut self) -> () {
        let mut node_store = self.store.borrow_mut();
        node_store.flush();
        let node_ctr = node_store.node_ctr();
        node_store.set_metadata(Metadata {
            fanout: S,
            root_node: self.root,
            block_size: BLOCK_SIZE,
            node_ctr,
            node_ident_size: size_of::<NodeIdent>(),
            search_key_size: size_of::<SearchKey>(),
        });
    }

    pub fn load(path: String) -> Result<Self, LoadError> {
        let (store, root) = match FileStore::<T, S>::load(path) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        Ok(Tree {
            root,
            store: Rc::new(RefCell::new(store)),
        })
    }

    pub fn new(path: String) -> Result<Self, io::Error> {
        let mut store = FileStore::<T, S>::new(path)?;

        let leaf = Node {
            children: [0; S],
            separators: [0; S],
            size: 0,
            phantom: PhantomData::<T>,
        };

        let root = store.store_node(leaf, true).unwrap();

        Ok(Self {
            root,
            store: Rc::new(RefCell::new(store)),
        })
    }

    pub fn print_graphviz(&self) {
        println!("digraph G {{");
        self.store.borrow_mut().print_stored_nodes(self.root);
        println!("}}");
    }

    pub fn search(&self, key: SearchKey) -> Result<Option<NodeIdent>, NodeStoreError> {
        Node::search(self.root, key, self.store.clone())
    }
}
