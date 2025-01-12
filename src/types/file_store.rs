use std::{
    fmt::Debug,
    fs::{File, OpenOptions},
    io,
    marker::PhantomData,
    os::unix::fs::FileExt,
};

use crate::types::node::{NodeIdent, SearchKey};

use super::{
    node::Node,
    node_store::{ByteSerialize, NodeStore, NodeStoreError, BLOCK_SIZE},
    second_chance_cache::{Cache, CacheItem},
};

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

#[derive(Debug)]
pub struct Metadata {
    pub fanout: usize,
    pub block_size: usize,
    pub node_ident_size: usize,
    pub search_key_size: usize,
    pub node_ctr: NodeIdent,
    pub root_node: NodeIdent,
}

impl ByteSerialize for Metadata {
    fn to_bytes(&self) -> [u8; BLOCK_SIZE] {
        let mut block = [0; BLOCK_SIZE];
        let mut index = 0;

        let rest = [
            self.fanout,
            self.block_size,
            self.node_ident_size,
            self.search_key_size,
        ];

        for &num in rest.iter() {
            let end = index + size_of::<usize>();
            block[index..end].copy_from_slice(&num.to_le_bytes());
            index = end;
        }

        let ctr_slice = &mut block[index..index + size_of::<NodeIdent>()];
        ctr_slice.copy_from_slice(&self.node_ctr.to_le_bytes());
        index += size_of::<NodeIdent>();

        let root_slice = &mut block[index..index + size_of::<NodeIdent>()];
        root_slice.copy_from_slice(&self.root_node.to_le_bytes());

        block
    }

    fn from_bytes(block: [u8; BLOCK_SIZE]) -> Self {
        let mut index = 0;
        let mut base_params = [0usize; 4];

        for i in 0..base_params.len() {
            let slice = &block[index..index + size_of::<usize>()];
            let mut entry = [0; size_of::<usize>()];
            entry.copy_from_slice(slice);

            base_params[i] = usize::from_le_bytes(entry);
            index += size_of::<usize>();
        }

        let ctr_slice = &block[index..index + size_of::<NodeIdent>()];
        let mut entry = [0u8; size_of::<NodeIdent>()];
        entry.copy_from_slice(ctr_slice);
        let node_ctr = NodeIdent::from_le_bytes(entry);
        index += size_of::<NodeIdent>();

        let root_slice = &block[index..index + size_of::<NodeIdent>()];
        let mut entry = [0u8; size_of::<NodeIdent>()];
        entry.copy_from_slice(root_slice);
        let root = NodeIdent::from_le_bytes(entry);

        let data = Metadata {
            fanout: base_params[0],
            block_size: base_params[1],
            node_ident_size: base_params[2],
            search_key_size: base_params[3],
            node_ctr,
            root_node: root,
        };
        data
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

#[derive(Debug)]
pub enum LoadError {
    ParameterMismatch,
    IOError(io::Error),
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

    pub fn load(file_name: String) -> Result<(Self, NodeIdent), LoadError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .append(false)
            .create(false)
            .open(file_name)
            .map_err(|e| LoadError::IOError(e))?;

        let mut buf = [0; BLOCK_SIZE];
        file.read_exact_at(&mut buf, 0)
            .map_err(|e| LoadError::IOError(e))?;

        let metadata = Metadata::from_bytes(buf);
        if metadata.block_size != BLOCK_SIZE
            || metadata.search_key_size != size_of::<SearchKey>()
            || metadata.node_ident_size != size_of::<NodeIdent>()
        {
            return Err(LoadError::ParameterMismatch);
        }

        Ok((
            FileStore::<T, S> {
                file,
                node_ctr: metadata.node_ctr,
                cache: Cache::<T, S>::new(),
            },
            metadata.root_node,
        ))
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

    fn set_metadata(&mut self, data: Metadata) {
        let block = data.to_bytes();
        self.set_block(0, block).unwrap();
    }

    fn node_ctr(&self) -> NodeIdent {
        self.node_ctr
    }
}
