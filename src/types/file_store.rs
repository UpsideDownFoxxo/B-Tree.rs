use std::{
    fmt::Debug,
    fs::{File, OpenOptions},
    marker::PhantomData,
    os::unix::fs::FileExt,
};

use crate::types::node::{NodeIdent, NodeRef, SearchKey};

use super::{
    node::{InnerNode, LeafNode, Node, NodeInstance},
    node_store::{NodeStore, NodeStoreError},
    second_chance_cache::Cache,
};

/// size of the file blocks in bytes
pub const BLOCK_SIZE: usize = 128;

pub trait ByteSerialize {
    fn to_bytes(&self) -> [u8; BLOCK_SIZE];
    fn from_bytes(block: [u8; BLOCK_SIZE]) -> Self;
}

impl<T, const S: usize> ByteSerialize for InnerNode<T, S>
where
    T: Sized,
    T: Debug,
{
    fn to_bytes(&self) -> [u8; BLOCK_SIZE] {
        let mut bytes: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
        let mut index = 0;

        for &key in &self.separators {
            let entry = key.to_le_bytes();
            let slice = &mut bytes[index..index + size_of::<SearchKey>()];
            slice.copy_from_slice(&entry);
            index += size_of::<SearchKey>();
        }
        for &ident in &self.children {
            let entry = ident.to_le_bytes();
            let slice = &mut bytes[index..index + size_of::<NodeIdent>()];
            slice.copy_from_slice(&entry);
            index += size_of::<NodeIdent>();
        }

        bytes
    }

    fn from_bytes(block: [u8; BLOCK_SIZE]) -> Self {
        let mut children: [NodeIdent; S] = [0; S];
        let mut separators: [SearchKey; S] = [0; S];

        let mut index = 0;
        for i in 0..separators.len() {
            let slice = &block[index..index + size_of::<SearchKey>()];
            let mut entry = [0; size_of::<SearchKey>()];
            entry.copy_from_slice(slice);

            separators[i] = SearchKey::from_le_bytes(entry);
            index += size_of::<SearchKey>();
        }

        for i in 0..children.len() {
            let slice = &block[index..index + size_of::<NodeIdent>()];
            let mut entry = [0; size_of::<NodeIdent>()];
            entry.copy_from_slice(slice);

            children[i] = NodeIdent::from_le_bytes(entry);
            index += size_of::<NodeIdent>();
        }

        let size = {
            let mut i = 0;
            while i < S && separators[i] != 0 {
                i += 1;
            }
            i
        };

        InnerNode {
            separators,
            children,
            size,
            phantom: PhantomData::<T>,
        }
    }
}

impl<T: Sized, const S: usize> ByteSerialize for LeafNode<T, S> {
    fn to_bytes(&self) -> [u8; BLOCK_SIZE] {
        let mut bytes: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
        let mut index = 0;

        for &key in &self.keys {
            let entry = key.to_le_bytes();
            let slice = &mut bytes[index..index + size_of::<SearchKey>()];
            slice.copy_from_slice(&entry);
            index += size_of::<SearchKey>();
        }
        for &ident in &self.data_blocks {
            let entry = ident.to_le_bytes();
            let slice = &mut bytes[index..index + size_of::<NodeIdent>()];
            slice.copy_from_slice(&entry);
            index += size_of::<NodeIdent>();
        }

        bytes
    }

    fn from_bytes(block: [u8; BLOCK_SIZE]) -> Self {
        let mut data_blocks: [NodeIdent; S] = [0; S];
        let mut keys: [SearchKey; S] = [0; S];

        let mut index = 0;
        for i in 0..keys.len() {
            let slice = &block[index..index + size_of::<SearchKey>()];
            let mut entry = [0; size_of::<SearchKey>()];
            entry.copy_from_slice(slice);

            keys[i] = SearchKey::from_le_bytes(entry);
            index += size_of::<SearchKey>();
        }

        for i in 0..data_blocks.len() {
            let slice = &block[index..index + size_of::<NodeIdent>()];
            let mut entry = [0; size_of::<NodeIdent>()];
            entry.copy_from_slice(slice);

            data_blocks[i] = NodeIdent::from_le_bytes(entry);
            index += size_of::<NodeIdent>();
        }

        let size = {
            let mut i = 0;
            while i < S && keys[i] != 0 {
                i += 1;
            }
            i
        };

        LeafNode {
            keys,
            data_blocks,
            size,
            phantom: PhantomData::<T>,
        }
    }
}

pub struct FileStore<T, const S: usize>
where
    T: Sized,
    T: Debug,
{
    current_inner_ident: Option<NodeIdent>,
    current_inner: InnerNode<T, S>,

    current_leaf_ident: Option<NodeIdent>,
    current_leaf: LeafNode<T, S>,
    file: File,
    node_ctr: NodeIdent,
    cache: Cache<T, S>,
}

impl<T, const S: usize> FileStore<T, S>
where
    T: Debug,
    T: Sized,
{
    pub fn new(file_name: String) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .append(false)
            .create(true)
            .open(file_name)?;

        Ok(FileStore::<T, S> {
            current_leaf_ident: None,
            current_inner_ident: None,
            file,
            current_inner: InnerNode {
                size: 0,
                separators: [0; S],
                children: [0; S],
                phantom: PhantomData::<T>,
            },
            current_leaf: LeafNode {
                size: 0,
                keys: [0; S],
                data_blocks: [0; S],
                phantom: PhantomData::<T>,
            },
            node_ctr: 0,
            cache: Cache::new(),
        })
    }

    pub fn get_block(&self, index: usize) -> Result<[u8; BLOCK_SIZE], NodeStoreError> {
        let mut buf = [0; BLOCK_SIZE];
        match self
            .file
            .read_exact_at(&mut buf, (BLOCK_SIZE * index) as u64)
        {
            Ok(()) => Ok(buf),
            Err(_) => Err(NodeStoreError::InvalidReference),
        }
    }

    pub fn set_block(&self, index: usize, block: [u8; BLOCK_SIZE]) -> Result<(), NodeStoreError> {
        match self.file.write_at(&block, (BLOCK_SIZE * index) as u64) {
            Ok(i) if i == BLOCK_SIZE => Ok(()),
            Ok(_i) => Err(NodeStoreError::WriteFailed),
            Err(_e) => Err(NodeStoreError::WriteFailed),
        }
    }
}

impl<T, const S: usize> NodeStore<T, S> for FileStore<T, S>
where
    T: Sized,
    T: Debug,
{
    fn get_node(&mut self, ident: NodeIdent) -> Result<super::node::NodeRef<T, S>, NodeStoreError> {
        let node = match self.cache.get_node(ident) {
            Some(n) => &n.node,
            None => {
                let mut removed = None;
                if ident < 0 {
                    let block = self.get_block((-ident) as usize)?;
                    let node: InnerNode<T, S> = InnerNode::from_bytes(block);
                    removed = self.cache.cache_node(ident, NodeInstance::Inner(node));
                } else {
                    let block = self.get_block(ident as usize)?;
                    let node: LeafNode<T, S> = LeafNode::from_bytes(block);
                    removed = self.cache.cache_node(ident, NodeInstance::Leaf(node));
                }

                if let Some((ident, node)) = removed {
                    match node {
                        NodeInstance::Inner(i) => {
                            let block = i.to_bytes();
                            self.set_block(-ident as usize, block);
                        }
                        NodeInstance::Leaf(l) => {
                            let block = l.to_bytes();
                            self.set_block(ident as usize, block);
                        }
                    }
                }

                // we inserted the node when calling cache, this should not fail
                &self.cache.get_node(ident).unwrap().node
            }
        };
        // page out currently stored node

        match node {
            NodeInstance::Leaf(l) => {
                self.current_leaf = l;
                Ok(NodeRef::Leaf(&mut self.current_leaf))
            }
            NodeInstance::Inner(i) => {
                self.current_inner = i;
                Ok(NodeRef::Inner(&mut self.current_inner))
            }
        }
    }

    fn store_node(
        &mut self,
        node: super::node::NodeInstance<T, S>,
    ) -> Result<NodeIdent, NodeStoreError> {
        self.node_ctr += 1;
        let ident = self.node_ctr;

        match node {
            NodeInstance::Inner(i) => {
                let block = i.to_bytes();
                self.set_block(ident as usize, block).map(|_| -ident)
            }
            NodeInstance::Leaf(l) => {
                let block = l.to_bytes();
                self.set_block(ident as usize, block).map(|_| ident)
            }
        }
    }

    fn print_stored_nodes(&mut self, root: NodeIdent) -> () {
        let mut node_stack: Vec<NodeIdent> = vec![root];
        while let Some(i) = node_stack.pop() {
            let node = self.get_node(i).unwrap();
            match node {
                NodeRef::Inner(inner) => {
                    println!("{:?}", inner.children);
                    inner
                        .children
                        .iter()
                        .filter(|e| **e != 0)
                        .for_each(|ch| node_stack.insert(0, ch.clone()));
                    println!("{}", inner.to_graphviz(&i));
                }
                NodeRef::Leaf(leaf) => println!("{}", leaf.to_graphviz(&i)),
            }
        }
        {}
    }
}
