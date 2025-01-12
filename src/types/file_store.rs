use std::{
    fmt::Debug,
    fs::{File, OpenOptions},
    marker::PhantomData,
    os::unix::fs::FileExt,
};

use crate::types::node::{NodeIdent, SearchKey};

use super::{
    node::{self, Node},
    node_store::{self, NodeStore, NodeStoreError},
    second_chance_cache::{Cache, CacheItem},
};

/// size of the file blocks in bytes
pub const BLOCK_SIZE: usize = 128;

pub trait ByteSerialize {
    fn to_bytes(&self) -> [u8; BLOCK_SIZE];
    fn from_bytes(block: [u8; BLOCK_SIZE]) -> Self;
}

impl<T, const S: usize> ByteSerialize for Node<T, S>
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

        Node {
            separators,
            children,
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
            file,
            node_ctr: 0,
            cache: Cache::<T, S>::new(),
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
    fn get_node(&mut self, ident: NodeIdent) -> Result<&mut Node<T, S>, NodeStoreError> {
        if self.cache.has_node(ident) {
            return Ok(&mut self.cache.get_node(ident).unwrap().node);
        }

        let block = { self.get_block(ident.abs() as usize)? };
        let node: Node<T, S> = Node::from_bytes(block);

        if let Some((ident, node)) = { self.cache.cache_node(ident, node) } {
            let block = node.to_bytes();
            self.set_block(ident.abs() as usize, block)?;
        }

        // we just inserted the node when calling cache, this should not fail
        Ok(&mut self.cache.get_node(ident).unwrap().node)
    }

    fn store_node(&mut self, node: Node<T, S>, is_leaf: bool) -> Result<NodeIdent, NodeStoreError> {
        self.node_ctr += 1;
        let ident = self.node_ctr;

        let block = node.to_bytes();
        self.set_block(ident as usize, block)
            .map(|_| if is_leaf { ident } else { -ident })
    }

    fn print_stored_nodes(&mut self, root: NodeIdent) -> () {
        let mut node_stack: Vec<NodeIdent> = vec![root];
        while let Some(i) = node_stack.pop() {
            let node = self.get_node(i).unwrap();
            println!("{}", node.to_graphviz(&i));
            if i < 0 {
                node.children[0..node.size + 1]
                    .iter()
                    .for_each(|i| node_stack.push(*i));
            }
        }
    }

    fn flush(&mut self) -> () {
        let nodes: Vec<(NodeIdent, CacheItem<T, S>)> = self.cache.drain().collect();
        nodes.iter().for_each(|(id, item)| {
            let node_block = item.node.to_bytes();
            self.set_block(id.abs() as usize, node_block).unwrap();
        });
    }
}
