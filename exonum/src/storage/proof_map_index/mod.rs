// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! An implementation of a Merkelized version of a map (Merkle Patricia tree).

use std::marker::PhantomData;
use std::fmt;

use crypto::{Hash, CryptoHash, HashStream};
use super::{BaseIndex, BaseIndexIter, Fork, Snapshot, StorageValue};
use self::key::{BitsRange, ChildKind, LEAF_KEY_PREFIX};
use self::node::{BranchNode, Node};
use self::proof::MapProofBuilder;

pub use self::key::{KEY_SIZE as PROOF_MAP_KEY_SIZE, ProofMapKey, HashedKey, ProofPath};
pub use self::proof::{MapProof, MapProofError};

#[cfg(test)]
mod tests;
mod key;
mod node;
mod proof;

/// A Merkelized version of a map that provides proofs of existence or non-existence for the map
/// keys.
///
/// `ProofMapIndex` implements a Merkle Patricia tree, storing the values as leaves.
/// `ProofMapIndex` requires that the keys implement [`ProofMapKey`] and values implement the
/// [`StorageValue`] trait.
///
/// **The size of the proof map keys must be exactly 32 bytes and the keys must have a uniform
/// distribution.** Usually [`Hash`] and [`PublicKey`] are used as types of proof map keys.
/// [`ProofMapKey`]: trait.ProofMapKey.html
/// [`StorageValue`]: ../trait.StorageValue.html
/// [`Hash`]: ../../crypto/struct.Hash.html
/// [`PublicKey`]: ../../crypto/struct.PublicKey.html
pub struct ProofMapIndex<T, K, V> {
    base: BaseIndex<T>,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

/// An iterator over the entries of a `ProofMapIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`ProofMapIndex`]. See its documentation for more.
///
/// [`iter`]: struct.ProofMapIndex.html#method.iter
/// [`iter_from`]: struct.ProofMapIndex.html#method.iter_from
/// [`ProofMapIndex`]: struct.ProofMapIndex.html
#[derive(Debug)]
pub struct ProofMapIndexIter<'a, K, V> {
    base_iter: BaseIndexIter<'a, ProofPath, V>,
    _k: PhantomData<K>,
}

/// An iterator over the keys of a `ProofMapIndex`.
///
/// This struct is created by the [`keys`] or
/// [`keys_from`] methods on [`ProofMapIndex`]. See its documentation for more.
///
/// [`keys`]: struct.ProofMapIndex.html#method.keys
/// [`keys_from`]: struct.ProofMapIndex.html#method.keys_from
/// [`ProofMapIndex`]: struct.ProofMapIndex.html
#[derive(Debug)]
pub struct ProofMapIndexKeys<'a, K> {
    base_iter: BaseIndexIter<'a, ProofPath, ()>,
    _k: PhantomData<K>,
}

/// An iterator over the values of a `ProofMapIndex`.
///
/// This struct is created by the [`values`] or
/// [`values_from`] methods on [`ProofMapIndex`]. See its documentation for more.
///
/// [`values`]: struct.ProofMapIndex.html#method.values
/// [`values_from`]: struct.ProofMapIndex.html#method.values_from
/// [`ProofMapIndex`]: struct.ProofMapIndex.html
#[derive(Debug)]
pub struct ProofMapIndexValues<'a, V> {
    base_iter: BaseIndexIter<'a, ProofPath, V>,
}

enum RemoveResult {
    KeyNotFound,
    Leaf,
    Branch((ProofPath, Hash)),
    UpdateHash(Hash),
}

impl<T, K, V> ProofMapIndex<T, K, V> {
    /// Creates a new index representation based on the name and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &snapshot);
    ///
    /// let mut fork = db.fork();
    /// let mut mut_index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &mut fork);
    /// # drop(index);
    /// # drop(mut_index);
    /// ```
    pub fn new<S: AsRef<str>>(name: S, view: T) -> Self {
        ProofMapIndex {
            base: BaseIndex::new(name, view),
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Creates a new index representation based on the name, common prefix of its keys
    /// and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let prefix = vec![01];
    ///
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> =
    ///                             ProofMapIndex::with_prefix(name, prefix.clone(), &snapshot);
    ///
    /// let mut fork = db.fork();
    /// let mut mut_index : ProofMapIndex<_, Hash, u8> =
    ///                                     ProofMapIndex::with_prefix(name, prefix, &mut fork);
    /// # drop(index);
    /// # drop(mut_index);
    /// ```
    pub fn with_prefix<S: AsRef<str>>(name: S, prefix: Vec<u8>, view: T) -> Self {
        ProofMapIndex {
            base: BaseIndex::with_prefix(name, prefix, view),
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<T, K, V> ProofMapIndex<T, K, V>
where
    T: AsRef<Snapshot>,
    K: ProofMapKey,
    V: StorageValue,
{
    fn get_root_path(&self) -> Option<ProofPath> {
        self.base.iter(&()).next().map(|(k, _): (ProofPath, ())| k)
    }

    fn get_root_node(&self) -> Option<(ProofPath, Node<V>)> {
        match self.get_root_path() {
            Some(key) => {
                let node = self.get_node_unchecked(&key);
                Some((key, node))
            }
            None => None,
        }
    }

    fn get_node_unchecked(&self, key: &ProofPath) -> Node<V> {
        // TODO: unwraps (ECR-84)?
        if key.is_leaf() {
            Node::Leaf(self.base.get(key).unwrap())
        } else {
            Node::Branch(self.base.get(key).unwrap())
        }
    }

    /// Returns the root hash of the proof map or default hash value if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &mut fork);
    ///
    /// let default_hash = index.root_hash();
    /// assert_eq!(Hash::default(), default_hash);
    ///
    /// index.put(&default_hash, 100);
    /// let hash = index.root_hash();
    /// assert_ne!(hash, default_hash);
    /// ```
    pub fn root_hash(&self) -> Hash {
        match self.get_root_node() {
            Some((k, Node::Leaf(v))) => {
                HashStream::new()
                    .update(k.as_bytes())
                    .update(v.hash().as_ref())
                    .hash()
            }
            Some((_, Node::Branch(branch))) => branch.hash(),
            None => Hash::zero(),
        }
    }

    /// Returns a value corresponding to the key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &mut fork);
    ///
    /// let hash = Hash::default();
    /// assert_eq!(None, index.get(&hash));
    ///
    /// index.put(&hash, 2);
    /// assert_eq!(Some(2), index.get(&hash));
    /// ```
    pub fn get(&self, key: &K) -> Option<V> {
        self.base.get(&ProofPath::new(key))
    }

    /// Returns `true` if the map contains a value for the specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &mut fork);
    ///
    /// let hash = Hash::default();
    /// assert!(!index.contains(&hash));
    ///
    /// index.put(&hash, 2);
    /// assert!(index.contains(&hash));
    /// ```
    pub fn contains(&self, key: &K) -> bool {
        self.base.contains(&ProofPath::new(key))
    }

    /// Returns the proof of existence or non-existence for the specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new("index", &snapshot);
    ///
    /// let proof = index.get_proof(Hash::default());
    /// # drop(proof);
    /// ```
    pub fn get_proof(&self, key: K) -> MapProof<K, V> {
        // How many key-hash pairs are expected to be on each side relative to the searched key
        const DEFAULT_CAPACITY: usize = 8;

        fn combine(
            mut left_hashes: Vec<(ProofPath, Hash)>,
            right_hashes: Vec<(ProofPath, Hash)>,
        ) -> Vec<(ProofPath, Hash)> {
            left_hashes.extend(right_hashes.into_iter().rev());
            left_hashes
        }

        let searched_path = ProofPath::new(&key);

        match self.get_root_node() {
            Some((root_path, Node::Branch(root_branch))) => {
                let mut left_hashes: Vec<(ProofPath, Hash)> = Vec::with_capacity(DEFAULT_CAPACITY);
                let mut right_hashes: Vec<(ProofPath, Hash)> = Vec::with_capacity(DEFAULT_CAPACITY);

                // Currently visited branch and its key, respectively
                let (mut branch, mut node_path) = (root_branch, root_path);

                // Do at least one loop, even if the supplied key does not match the root key.
                // This is necessary to put both children of the root node into the proof
                // in this case.
                loop {
                    // <256 by induction; `branch` is always a branch node, and `node_path`
                    // is its key
                    let next_height = node_path.len();
                    let next_bit = searched_path.bit(next_height);
                    node_path = branch.child_path(next_bit);

                    // XXX: strictly speaking, one of `*branch.child_hash()` copies could
                    // be avoided by dismatling `branch` via a consuming method
                    let other_path_and_hash =
                        (branch.child_path(!next_bit), *branch.child_hash(!next_bit));
                    match !next_bit {
                        ChildKind::Left => left_hashes.push(other_path_and_hash),
                        ChildKind::Right => right_hashes.push(other_path_and_hash),
                    }

                    if !searched_path.matches_from(&node_path, next_height) {
                        // Both children of `branch` do not fit

                        let next_hash = *branch.child_hash(next_bit);
                        match next_bit {
                            ChildKind::Left => left_hashes.push((node_path, next_hash)),
                            ChildKind::Right => right_hashes.push((node_path, next_hash)),
                        }

                        return MapProof::for_absent_key(key, combine(left_hashes, right_hashes));
                    } else {
                        let node = self.get_node_unchecked(&node_path);
                        match node {
                            Node::Branch(branch_) => branch = branch_,
                            Node::Leaf(value) => {
                                // We have reached the leaf node and haven't diverged!
                                // The key is there, we've just gotten the value, so we just
                                // need to return it.
                                return MapProof::for_entry(
                                    (key, value),
                                    combine(left_hashes, right_hashes),
                                );
                            }
                        }
                    }
                }
            }

            Some((root_path, Node::Leaf(root_value))) => {
                if root_path == searched_path {
                    MapProof::for_entry((key, root_value), vec![])
                } else {
                    MapProof::for_absent_key(key, vec![(root_path, root_value.hash())])
                }
            }

            None => MapProof::for_empty_map(vec![key]),
        }
    }

    /// Returns the combined proof of existence or non-existence for the multiple specified keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, [u8; 32], u8> = ProofMapIndex::new("index", &snapshot);
    ///
    /// let proof = index.get_multiproof(vec![[0; 32], [1; 32]]);
    /// # drop(proof);
    /// ```
    pub fn get_multiproof<KI>(&self, keys: KI) -> MapProof<K, V>
    where
        KI: IntoIterator<Item = K>,
    {
        const CONTOUR_CAPACITY: usize = 8;

        #[derive(Debug)]
        struct ContourNode {
            key: ProofPath,
            branch: BranchNode,
            visited_left: bool,
            visited_right: bool,
        }

        impl ContourNode {
            fn new(key: ProofPath, branch: BranchNode) -> Self {
                ContourNode {
                    key,
                    branch,
                    visited_left: false,
                    visited_right: false,
                }
            }

            // Adds this contour node into a proof builder.
            fn add_to_proof<K, V>(
                self,
                mut builder: MapProofBuilder<K, V>,
            ) -> MapProofBuilder<K, V> {
                if !self.visited_right {
                    // This works due to the following observation: If neither of the child nodes
                    // were visited when the node is being ejected from the contour,
                    // this means that it is safe to add the left and right hashes (in this order)
                    // to the proof. The observation is provable by induction.
                    if !self.visited_left {
                        builder = builder.add_proof_entry(
                            self.branch.child_path(ChildKind::Left),
                            *self.branch.child_hash(ChildKind::Left),
                        );
                    }

                    builder = builder.add_proof_entry(
                        self.branch.child_path(ChildKind::Right),
                        *self.branch.child_hash(ChildKind::Right),
                    );
                }

                builder
            }
        }

        // // // // Processing for a single key in a map with multiple entries // // // //

        fn process_key<K, V, F>(
            contour: &mut Vec<ContourNode>,
            mut builder: MapProofBuilder<K, V>,
            proof_path: &ProofPath,
            key: K,
            lookup: F,
        ) -> MapProofBuilder<K, V>
        where
            V: StorageValue,
            F: Fn(&ProofPath) -> Node<V>,
        {
            // `unwrap()` is safe: there is at least 1 element in the contour by design
            let common_prefix = proof_path.common_prefix_len(&contour.last().unwrap().key);

            // Eject nodes from the contour while they will they can be "finalized"
            while let Some(node) = contour.pop() {
                if contour.is_empty() || node.key.len() <= common_prefix {
                    contour.push(node);
                    break;
                } else {
                    builder = node.add_to_proof(builder);
                }
            }

            // Push new items to the contour
            'traverse: loop {
                let node_path = {
                    let contour_tip = contour.last_mut().unwrap();

                    let next_height = contour_tip.key.len();
                    let next_bit = proof_path.bit(next_height);
                    let node_path = contour_tip.branch.child_path(next_bit);

                    if !proof_path.matches_from(&node_path, next_height) {
                        // Both children of `branch` do not fit; stop here
                        builder = builder.add_missing(key);
                        break 'traverse;
                    } else {
                        match next_bit {
                            ChildKind::Left => contour_tip.visited_left = true,
                            ChildKind::Right => {
                                if !contour_tip.visited_left {
                                    builder =
                                        builder.add_proof_entry(
                                            contour_tip.branch.child_path(ChildKind::Left),
                                            *contour_tip.branch.child_hash(ChildKind::Left),
                                        );
                                }
                                contour_tip.visited_right = true;
                            }
                        }

                        node_path
                    }
                };

                let node = lookup(&node_path);
                match node {
                    Node::Branch(branch) => {
                        contour.push(ContourNode::new(node_path, branch));
                    }

                    Node::Leaf(value) => {
                        // We have reached the leaf node and haven't diverged!
                        builder = builder.add_entry(key, value);
                        break 'traverse;
                    }
                }
            }

            builder
        }

        // // // // `get_multiproof()` main section // // // //

        match self.get_root_node() {
            Some((root_path, Node::Branch(root_branch))) => {
                let mut builder = MapProofBuilder::new();

                let searched_paths: Vec<_> = {
                    let mut keys: Vec<_> =
                        keys.into_iter().map(|k| (ProofPath::new(&k), k)).collect();

                    keys.sort_by(|x, y| {
                        // `unwrap` is safe here because all keys start from the same position `0`
                        x.0.partial_cmp(&y.0).unwrap()
                    });
                    keys
                };

                let mut contour = Vec::with_capacity(CONTOUR_CAPACITY);
                contour.push(ContourNode::new(root_path, root_branch));

                for (proof_path, key) in searched_paths {
                    builder = process_key(&mut contour, builder, &proof_path, key, |key| {
                        self.get_node_unchecked(key)
                    });
                }

                // Eject remaining entries from the contour
                while let Some(node) = contour.pop() {
                    builder = node.add_to_proof(builder);
                }

                builder.create()
            }

            Some((root_path, Node::Leaf(root_value))) => {
                let mut builder = MapProofBuilder::new();
                // (One of) keys corresponding to the existing table entry.
                let mut found_key: Option<K> = None;

                for key in keys {
                    let searched_path = ProofPath::new(&key);
                    if root_path == searched_path {
                        found_key = Some(key);
                    } else {
                        builder = builder.add_missing(key);
                    }
                }

                builder = if let Some(key) = found_key {
                    builder.add_entry(key, root_value)
                } else {
                    builder.add_proof_entry(root_path, root_value.hash())
                };

                builder.create()
            }

            None => MapProof::for_empty_map(keys),
        }
    }

    /// Returns an iterator over the entries of the map in ascending order. The iterator element
    /// type is `(K::Output, V)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &snapshot);
    ///
    /// for val in index.iter() {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter(&self) -> ProofMapIndexIter<K, V> {
        ProofMapIndexIter {
            base_iter: self.base.iter(&LEAF_KEY_PREFIX),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the keys of the map in ascending order. The iterator element
    /// type is `K::Output`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &snapshot);
    ///
    /// for key in index.keys() {
    ///     println!("{:?}", key);
    /// }
    /// ```
    pub fn keys(&self) -> ProofMapIndexKeys<K> {
        ProofMapIndexKeys {
            base_iter: self.base.iter(&LEAF_KEY_PREFIX),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the values of the map in ascending order of keys. The iterator
    /// element type is `V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &snapshot);
    ///
    /// for val in index.values() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values(&self) -> ProofMapIndexValues<V> {
        ProofMapIndexValues { base_iter: self.base.iter(&LEAF_KEY_PREFIX) }
    }

    /// Returns an iterator over the entries of the map in ascending order starting from the
    /// specified key. The iterator element type is `(K::Output, V)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &snapshot);
    ///
    /// let hash = Hash::default();
    /// for val in index.iter_from(&hash) {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: &K) -> ProofMapIndexIter<K, V> {
        ProofMapIndexIter {
            base_iter: self.base.iter_from(&LEAF_KEY_PREFIX, &ProofPath::new(from)),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the keys of the map in ascending order starting from the
    /// specified key. The iterator element type is `K::Output`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &snapshot);
    ///
    /// let hash = Hash::default();
    /// for key in index.keys_from(&hash) {
    ///     println!("{:?}", key);
    /// }
    /// ```
    pub fn keys_from(&self, from: &K) -> ProofMapIndexKeys<K> {
        ProofMapIndexKeys {
            base_iter: self.base.iter_from(&LEAF_KEY_PREFIX, &ProofPath::new(from)),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the values of the map in ascending order of keys starting from the
    /// specified key. The iterator element type is `V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &snapshot);
    ///
    /// let hash = Hash::default();
    /// for val in index.values_from(&hash) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values_from(&self, from: &K) -> ProofMapIndexValues<V> {
        ProofMapIndexValues {
            base_iter: self.base.iter_from(&LEAF_KEY_PREFIX, &ProofPath::new(from)),
        }
    }
}

impl<'a, K, V> ProofMapIndex<&'a mut Fork, K, V>
where
    K: ProofMapKey,
    V: StorageValue,
{
    fn insert_leaf(&mut self, key: &ProofPath, value: V) -> Hash {
        debug_assert!(key.is_leaf());
        let hash = value.hash();
        self.base.put(key, value);
        hash
    }

    // Inserts a new node as child of current branch and returns updated hash
    // or if a new node has more short key returns a new key length
    fn insert_branch(
        &mut self,
        parent: &BranchNode,
        proof_path: &ProofPath,
        value: V,
    ) -> (Option<u16>, Hash) {
        let child_path = parent.child_path(proof_path.bit(0)).start_from(
            proof_path.start(),
        );
        // If the path is fully fit in key then there is a two cases
        let i = child_path.common_prefix_len(proof_path);
        if child_path.len() == i {
            // check that child is leaf to avoid unnecessary read
            if child_path.is_leaf() {
                // there is a leaf in branch and we needs to update its value
                let hash = self.insert_leaf(proof_path, value);
                (None, hash)
            } else {
                match self.get_node_unchecked(&child_path) {
                    Node::Leaf(_) => {
                        unreachable!("Something went wrong!");
                    }
                    // There is a child in branch and we needs to lookup it recursively
                    Node::Branch(mut branch) => {
                        let (j, h) = self.insert_branch(&branch, &proof_path.suffix(i), value);
                        match j {
                            Some(j) => {
                                branch.set_child(
                                    proof_path.bit(i),
                                    &proof_path.suffix(i).prefix(j),
                                    &h,
                                );
                            }
                            None => branch.set_child_hash(proof_path.bit(i), &h),
                        };
                        let hash = branch.hash();
                        self.base.put(&child_path, branch);
                        (None, hash)
                    }
                }
            }
        } else {
            // A simple case of inserting a new branch
            let suffix_path = proof_path.suffix(i);
            let mut new_branch = BranchNode::empty();
            // Add a new leaf
            let hash = self.insert_leaf(&suffix_path, value);
            new_branch.set_child(suffix_path.bit(0), &suffix_path, &hash);
            // Move current branch
            new_branch.set_child(
                child_path.bit(i),
                &child_path.suffix(i),
                parent.child_hash(proof_path.bit(0)),
            );

            let hash = new_branch.hash();
            self.base.put(&proof_path.prefix(i), new_branch);
            (Some(i), hash)
        }
    }

    /// Inserts the key-value pair into the proof map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &mut fork);
    ///
    /// let hash = Hash::default();
    /// index.put(&hash, 2);
    /// assert!(index.contains(&hash));
    /// ```
    pub fn put(&mut self, key: &K, value: V) {
        let proof_path = ProofPath::new(key);
        match self.get_root_node() {
            Some((prefix, Node::Leaf(prefix_data))) => {
                let prefix_path = prefix;
                let i = prefix_path.common_prefix_len(&proof_path);

                let leaf_hash = self.insert_leaf(&proof_path, value);
                if i < proof_path.len() {
                    let mut branch = BranchNode::empty();
                    branch.set_child(proof_path.bit(i), &proof_path.suffix(i), &leaf_hash);
                    branch.set_child(
                        prefix_path.bit(i),
                        &prefix_path.suffix(i),
                        &prefix_data.hash(),
                    );
                    let new_prefix = proof_path.prefix(i);
                    self.base.put(&new_prefix, branch);
                }
            }
            Some((prefix, Node::Branch(mut branch))) => {
                let prefix_path = prefix;
                let i = prefix_path.common_prefix_len(&proof_path);

                if i == prefix_path.len() {
                    let suffix_path = proof_path.suffix(i);
                    // Just cut the prefix and recursively descent on.
                    let (j, h) = self.insert_branch(&branch, &suffix_path, value);
                    match j {
                        Some(j) => branch.set_child(suffix_path.bit(0), &suffix_path.prefix(j), &h),
                        None => branch.set_child_hash(suffix_path.bit(0), &h),
                    };
                    self.base.put(&prefix_path, branch);
                } else {
                    // Inserts a new branch and adds current branch as its child
                    let hash = self.insert_leaf(&proof_path, value);
                    let mut new_branch = BranchNode::empty();
                    new_branch.set_child(
                        prefix_path.bit(i),
                        &prefix_path.suffix(i),
                        &branch.hash(),
                    );
                    new_branch.set_child(proof_path.bit(i), &proof_path.suffix(i), &hash);
                    // Saves a new branch
                    let new_prefix = prefix_path.prefix(i);
                    self.base.put(&new_prefix, new_branch);
                }
            }
            None => {
                self.insert_leaf(&proof_path, value);
            }
        }
    }

    fn remove_node(&mut self, parent: &BranchNode, proof_path: &ProofPath) -> RemoveResult {
        let child_path = parent.child_path(proof_path.bit(0)).start_from(
            proof_path.start(),
        );
        let i = child_path.common_prefix_len(proof_path);

        if i == child_path.len() {
            match self.get_node_unchecked(&child_path) {
                Node::Leaf(_) => {
                    self.base.remove(proof_path);
                    return RemoveResult::Leaf;
                }
                Node::Branch(mut branch) => {
                    let suffix_path = proof_path.suffix(i);
                    match self.remove_node(&branch, &suffix_path) {
                        RemoveResult::Leaf => {
                            let child = !suffix_path.bit(0);
                            let key = branch.child_path(child);
                            let hash = branch.child_hash(child);

                            self.base.remove(&child_path);

                            return RemoveResult::Branch((key, *hash));
                        }
                        RemoveResult::Branch((key, hash)) => {
                            let new_child_path = key.start_from(suffix_path.start());

                            branch.set_child(suffix_path.bit(0), &new_child_path, &hash);
                            let h = branch.hash();
                            self.base.put(&child_path, branch);
                            return RemoveResult::UpdateHash(h);
                        }
                        RemoveResult::UpdateHash(hash) => {
                            branch.set_child_hash(suffix_path.bit(0), &hash);
                            let h = branch.hash();
                            self.base.put(&child_path, branch);
                            return RemoveResult::UpdateHash(h);
                        }
                        RemoveResult::KeyNotFound => return RemoveResult::KeyNotFound,
                    }
                }
            }
        }
        RemoveResult::KeyNotFound
    }

    /// Removes the key from the proof map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &mut fork);
    ///
    /// let hash = Hash::default();
    /// index.put(&hash, 2);
    /// assert!(index.contains(&hash));
    ///
    /// index.remove(&hash);
    /// assert!(!index.contains(&hash));
    /// ```
    pub fn remove(&mut self, key: &K) {
        let proof_path = ProofPath::new(key);
        match self.get_root_node() {
            // If we have only on leaf, then we just need to remove it (if any)
            Some((prefix, Node::Leaf(_))) => {
                let key = proof_path;
                if key == prefix {
                    self.base.remove(&key);
                }
            }
            Some((prefix, Node::Branch(mut branch))) => {
                // Truncate prefix
                let i = prefix.common_prefix_len(&proof_path);
                if i == prefix.len() {
                    let suffix_path = proof_path.suffix(i);
                    match self.remove_node(&branch, &suffix_path) {
                        RemoveResult::Leaf => self.base.remove(&prefix),
                        RemoveResult::Branch((key, hash)) => {
                            let new_child_path = key.start_from(suffix_path.start());
                            branch.set_child(suffix_path.bit(0), &new_child_path, &hash);
                            self.base.put(&prefix, branch);
                        }
                        RemoveResult::UpdateHash(hash) => {
                            branch.set_child_hash(suffix_path.bit(0), &hash);
                            self.base.put(&prefix, branch);
                        }
                        RemoveResult::KeyNotFound => return,
                    }
                }
            }
            None => (),
        }
    }

    /// Clears the proof map, removing all entries.
    ///
    /// # Notes
    ///
    /// Currently this method is not optimized to delete large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofMapIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &mut fork);
    ///
    /// let hash = Hash::default();
    /// index.put(&hash, 2);
    /// assert!(index.contains(&hash));
    ///
    /// index.clear();
    /// assert!(!index.contains(&hash));
    /// ```
    pub fn clear(&mut self) {
        self.base.clear()
    }
}

impl<'a, T, K, OK, V> ::std::iter::IntoIterator for &'a ProofMapIndex<T, K, V>
where
    T: AsRef<Snapshot>,
    K: ProofMapKey<Output = OK>,
    OK: ProofMapKey,
    V: StorageValue,
{
    type Item = (OK, V);
    type IntoIter = ProofMapIndexIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, OK, V> Iterator for ProofMapIndexIter<'a, K, V>
where
    K: ProofMapKey<Output = OK>,
    OK: ProofMapKey,
    V: StorageValue,
{
    type Item = (OK, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(
            |(k, v)| (K::read_key(k.raw_key()), v),
        )
    }
}


impl<'a, K, OK> Iterator for ProofMapIndexKeys<'a, K>
where
    K: ProofMapKey<Output = OK>,
    OK: ProofMapKey,
{
    type Item = OK;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, _)| K::read_key(k.raw_key()))
    }
}

impl<'a, V> Iterator for ProofMapIndexValues<'a, V>
where
    V: StorageValue,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(_, v)| v)
    }
}

impl<T, K, V> fmt::Debug for ProofMapIndex<T, K, V>
where
    T: AsRef<Snapshot>,
    K: ProofMapKey,
    V: StorageValue + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct Entry<'a, T: 'a, K: 'a, V: 'a + StorageValue> {
            index: &'a ProofMapIndex<T, K, V>,
            path: ProofPath,
            hash: Hash,
            node: Node<V>,
        }

        impl<'a, T, K, V> Entry<'a, T, K, V>
        where
            T: AsRef<Snapshot>,
            K: ProofMapKey,
            V: StorageValue,
        {
            fn new(index: &'a ProofMapIndex<T, K, V>, hash: Hash, path: ProofPath) -> Self {
                Entry {
                    index,
                    path,
                    hash,
                    node: index.get_node_unchecked(&path),
                }
            }

            fn child(&self, self_branch: &BranchNode, kind: ChildKind) -> Self {
                Self::new(
                    self.index,
                    *self_branch.child_hash(kind),
                    self_branch.child_path(kind),
                )
            }
        }

        impl<'a, T, K, V> fmt::Debug for Entry<'a, T, K, V>
        where
            T: AsRef<Snapshot>,
            K: ProofMapKey,
            V: StorageValue + fmt::Debug,
        {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self.node {
                    Node::Leaf(ref value) => {
                        f.debug_struct("Leaf")
                            .field("key", &self.path)
                            .field("hash", &self.hash)
                            .field("value", value)
                            .finish()
                    }
                    Node::Branch(ref branch) => {
                        f.debug_struct("Branch")
                            .field("path", &self.path)
                            .field("hash", &self.hash)
                            .field("left", &self.child(branch, ChildKind::Left))
                            .field("right", &self.child(branch, ChildKind::Right))
                            .finish()
                    }
                }

            }
        }

        if let Some(prefix) = self.get_root_path() {
            let root_entry = Entry::new(self, self.root_hash(), prefix);
            f.debug_struct("ProofMapIndex")
                .field("entries", &root_entry)
                .finish()
        } else {
            f.debug_struct("ProofMapIndex").finish()
        }
    }
}
