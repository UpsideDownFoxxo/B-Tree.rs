use std::{
    collections::{hash_map::Drain, HashMap},
    fmt::Debug,
};

use super::node::{Node, NodeIdent};

const CACHE_SIZE: usize = 4;

pub struct CacheItem<T, const S: usize>
where
    T: Debug,
    T: Sized,
{
    pub node: Node<T, S>,
    chances: u8,
}

pub struct Cache<T, const S: usize>
where
    T: Debug,
    T: Sized,
{
    nodes: HashMap<NodeIdent, CacheItem<T, S>>,
}

impl<T, const S: usize> Cache<T, S>
where
    T: Sized,
    T: Debug,
{
    pub fn has_node(&mut self, node: NodeIdent) -> bool {
        self.nodes.contains_key(&node)
    }
    /// gets an already present node from the cache and bumps its chances
    pub fn get_node(&mut self, node: NodeIdent) -> Option<&mut CacheItem<T, S>> {
        match self.nodes.get_mut(&node) {
            Some(e) => {
                // limit the amount of chances we give each node before it gets paged out
                if e.chances < 8 {
                    e.chances += 1;
                }
                Some(e)
            }
            None => None,
        }
    }

    /// takes in a node and caches it. May return a value displaced by the operation. This value
    /// can be considered unused and should be moved to long-term storage
    pub fn cache_node(
        &mut self,
        ident: NodeIdent,
        node: Node<T, S>,
    ) -> Option<(NodeIdent, Node<T, S>)> {
        let mut ret = None;
        if self.nodes.len() == CACHE_SIZE {
            // page out

            let remove = 'outer: loop {
                let mut it = self.nodes.iter_mut();
                loop {
                    let next = it.next();
                    if let Some(i) = next {
                        if i.1.chances > 0 {
                            i.1.chances -= 1;
                            continue;
                        } else {
                            break 'outer i.0.clone();
                        }
                    } else {
                        break;
                    }
                }
            };

            // we checked for existence from within the iterator
            ret = self.nodes.remove(&remove).map(|i| (remove, i.node));
        }

        self.nodes.insert(ident, CacheItem { node, chances: 1 });
        ret
    }

    pub fn drain(&mut self) -> Drain<NodeIdent, CacheItem<T, S>> {
        self.nodes.drain()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }
}
