# B+ Tree

A B+ Tree written in Rust. Nodes are pulled from an underlying file and are cached for better performance.

## Components

### Tree

The `Tree` provides a wrapper for the different subcomponents of the B+ tree. It consists of an instance of a `NodeStore` and a reference to the root node.
It would be the main thing to interact with if you wanted to use this in a program (for whatever reason)

### NodeStore

The `NodeStore` manages loading/saving the nodes to disk. The B+ tree itself only holds ONE reference to any of its nodes,
and will only ask for a new node when the operation with the current node has finished.
The provided Store also implements a Second(ish)-Chance:TM: cache to avoid unnecessary write operations for frequently used nodes.
A reference to the trees NodeStore is passed down recursively along the search path.

### Node

A `Node<T,S>` consists of two arrays of length `S`. One to store the search keys separating the subtrees, and one to store references to the subtrees.
In the provided example `S` is dynamically calculated according to the specified `BLOCK_SIZE` and the sizes of the search keys and references.
_(Note: `S` was intended to be even. It is assumed when splitting the nodes, and might (will!) cause weirdness if it's not)_

## Visuals

The `Tree` provides a function to print the stored nodes in the DOT format. Use this to inspect the created trees.
I can recommend [GraphvizOnline](https://dreampuf.github.io/GraphvizOnline/?engine=dot) for this.

## Building

The final version of the project was built with the nightly rust compiler (rustc 1.86.0-nightly), since it uses an experimental feature to perform operations on constants at compile time.

## Closing Thoughts

I had a bit more ambitious plans when starting this project, but decided to abandon them due to time constraints (and some skill issues on my part).
Maybe insisting on doing this in Rust wasn't such a great idea. I'm quite new to the language, and severely underestimated some of the challenges
that came from a "simple" tree, but I guess that's why this exercise existed in the first place. I probably won't forget how to split nodes anytime soon.

### CLI

The current way of interacting with the tree, changing code and recompiling, is not very fun. A CLI would have been nice,
since you'd get to actually "play" and experiment with the tree and not just watch it insert random values.

### Ability to actually store data

Currently the Tree only stores references to nodes. This means another data structure is necessary to actually use it beyond storing integers.

### Delete operations

Adding delete to the tree would be nice, adding it to the file store wouldn't. At least not without storing deleted nodes. In the lecture
it was mentioned that in practice deletions are rare, so maybe this isn't that big of a problem, but it does feel a bit wrong.

### Separate types for leaves

This one definitely cost me the most time. Leaf nodes are currently constrained to have the same value as the node identifier,
which means the "stored" ""data"" has the same type as the node identifiers used internally.
However from my fights with the borrow checker I eventually learned that all nodes have to be instances of the same struct to avoid 💥.
Ideally I would want to have separate types for leaves and inner nodes to allow for more flexibility, but alas.
