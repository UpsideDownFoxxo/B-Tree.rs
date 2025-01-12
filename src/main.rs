#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use random::Source;
use types::{
    node::{NodeIdent, SearchKey},
    node_store::BLOCK_SIZE,
    tree::Tree,
};

pub mod types;

const SIZE_PER_ENTRY: usize = size_of::<SearchKey>() + size_of::<NodeIdent>();
const FANOUT: usize = (BLOCK_SIZE / SIZE_PER_ENTRY) / 2 * 2;

fn main() {
    let mut tree = Tree::<i64, FANOUT>::new("test_tree2".to_string()).unwrap();
    let mut insert = random::default(0);

    for _i in 1..200 {
        let key = insert.read_u64() as i64;
        tree.insert(key, 10);
    }

    tree.insert(10, 5);
    println!("{:?}", tree.search(10));

    tree.save();
    tree.print_graphviz();

    let tree2 = Tree::<i64, FANOUT>::load("test_tree2".to_string()).unwrap();
    tree2.print_graphviz();
}
