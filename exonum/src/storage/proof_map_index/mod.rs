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

//! An implementation of a Merklized version of a map (Merkle Patricia tree).
use std::marker::PhantomData;

use crypto::{Hash, HashStream};

use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageValue};

use self::key::{DBKey, ChildKind, LEAF_KEY_PREFIX};
use self::node::{Node, BranchNode};

pub use self::key::{ProofMapKey, KEY_SIZE as PROOF_MAP_KEY_SIZE, DBKey as ProofMapDBKey};
pub use self::proof::{MapProof, ProofNode, BranchProofNode};

#[cfg(test)]
mod tests;
mod key;
mod node;
mod proof;

/// A Merkalized version of a map that provides proofs of existence or non-existence for the map
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
#[derive(Debug)]
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
    base_iter: BaseIndexIter<'a, DBKey, V>,
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
    base_iter: BaseIndexIter<'a, DBKey, ()>,
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
    base_iter: BaseIndexIter<'a, DBKey, V>,
}

enum RemoveResult {
    KeyNotFound,
    Leaf,
    Branch((DBKey, Hash)),
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
    fn get_root_key(&self) -> Option<DBKey> {
        self.base.iter(&()).next().map(|(k, _): (DBKey, ())| k)
    }

    fn get_root_node(&self) -> Option<(DBKey, Node<V>)> {
        match self.get_root_key() {
            Some(key) => {
                let node = self.get_node_unchecked(&key);
                Some((key, node))
            }
            None => None,
        }
    }

    fn get_node_unchecked(&self, key: &DBKey) -> Node<V> {
        // TODO: unwraps (ECR-84)?
        if key.is_leaf() {
            Node::Leaf(self.base.get(key).unwrap())
        } else {
            Node::Branch(self.base.get(key).unwrap())
        }
    }

    fn construct_proof(
        &self,
        current_branch: &BranchNode,
        searched_slice: &DBKey,
    ) -> Option<ProofNode<V>> {

        let mut child_slice = current_branch.child_slice(searched_slice.get(0));
        child_slice.set_from(searched_slice.from());
        let c_pr_l = child_slice.common_prefix(searched_slice);
        debug_assert!(c_pr_l > 0);
        if c_pr_l < child_slice.len() {
            return None;
        }

        let res: ProofNode<V> = match self.get_node_unchecked(&child_slice) {
            Node::Leaf(child_value) => ProofNode::Leaf(child_value),
            Node::Branch(child_branch) => {
                let l_s = child_branch.child_slice(ChildKind::Left);
                let r_s = child_branch.child_slice(ChildKind::Right);
                let suf_searched_slice = searched_slice.suffix(c_pr_l);
                let proof_from_level_below: Option<ProofNode<V>> =
                    self.construct_proof(&child_branch, &suf_searched_slice);

                if let Some(child_proof) = proof_from_level_below {
                    let child_proof_pos = suf_searched_slice.get(0);
                    let neighbour_child_hash = *child_branch.child_hash(!child_proof_pos);
                    match child_proof_pos {
                        ChildKind::Left => {
                            ProofNode::Branch(BranchProofNode::LeftBranch {
                                left_node: Box::new(child_proof),
                                right_hash: neighbour_child_hash,
                                left_key: l_s.suffix(searched_slice.from() + c_pr_l),
                                right_key: r_s.suffix(searched_slice.from() + c_pr_l),
                            })
                        }
                        ChildKind::Right => {
                            ProofNode::Branch(BranchProofNode::RightBranch {
                                left_hash: neighbour_child_hash,
                                right_node: Box::new(child_proof),
                                left_key: l_s.suffix(searched_slice.from() + c_pr_l),
                                right_key: r_s.suffix(searched_slice.from() + c_pr_l),
                            })
                        }
                    }
                } else {
                    let l_h = *child_branch.child_hash(ChildKind::Left); //copy
                    let r_h = *child_branch.child_hash(ChildKind::Right); //copy
                    ProofNode::Branch(BranchProofNode::BranchKeyNotFound {
                        left_hash: l_h,
                        right_hash: r_h,
                        left_key: l_s.suffix(searched_slice.from() + c_pr_l),
                        right_key: r_s.suffix(searched_slice.from() + c_pr_l),
                    })
                    // proof of exclusion of a key, because none of child slices is a
                    // prefix(searched_slice)
                }
            }
        };
        Some(res)
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
                    .update(&k.as_bytes())
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
        self.base.get(&DBKey::leaf(key))
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
        self.base.contains(&DBKey::leaf(key))
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
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &snapshot);
    ///
    /// let hash = Hash::default();
    /// let proof = index.get_proof(&hash);
    /// # drop(proof);
    /// ```
    pub fn get_proof(&self, key: &K) -> MapProof<V> {
        let searched_slice = DBKey::leaf(key);

        match self.get_root_node() {
            Some((root_db_key, Node::Leaf(root_value))) => {
                if searched_slice == root_db_key {
                    MapProof::LeafRootInclusive(root_db_key, root_value)
                } else {
                    MapProof::LeafRootExclusive(root_db_key, root_value.hash())
                }
            }
            Some((root_db_key, Node::Branch(branch))) => {
                let root_slice = root_db_key;
                let l_s = branch.child_slice(ChildKind::Left);
                let r_s = branch.child_slice(ChildKind::Right);

                let c_pr_l = root_slice.common_prefix(&searched_slice);
                if c_pr_l == root_slice.len() {
                    let suf_searched_slice = searched_slice.suffix(c_pr_l);
                    let proof_from_level_below: Option<ProofNode<V>> =
                        self.construct_proof(&branch, &suf_searched_slice);

                    if let Some(child_proof) = proof_from_level_below {
                        let child_proof_pos = suf_searched_slice.get(0);
                        let neighbour_child_hash = *branch.child_hash(!child_proof_pos);
                        match child_proof_pos {
                            ChildKind::Left => {
                                MapProof::Branch(BranchProofNode::LeftBranch {
                                    left_node: Box::new(child_proof),
                                    right_hash: neighbour_child_hash,
                                    left_key: l_s,
                                    right_key: r_s,
                                })
                            }
                            ChildKind::Right => {
                                MapProof::Branch(BranchProofNode::RightBranch {
                                    left_hash: neighbour_child_hash,
                                    right_node: Box::new(child_proof),
                                    left_key: l_s,
                                    right_key: r_s,
                                })
                            }
                        }
                    } else {
                        let l_h = *branch.child_hash(ChildKind::Left); //copy
                        let r_h = *branch.child_hash(ChildKind::Right); //copy
                        MapProof::Branch(BranchProofNode::BranchKeyNotFound {
                            left_hash: l_h,
                            right_hash: r_h,
                            left_key: l_s,
                            right_key: r_s,
                        })
                        // proof of exclusion of a key, because none of child slices is a
                        // prefix(searched_slice)
                    }
                } else {
                    // if common prefix length with root_slice is less than root_slice length
                    let l_h = *branch.child_hash(ChildKind::Left); //copy
                    let r_h = *branch.child_hash(ChildKind::Right); //copy
                    MapProof::Branch(BranchProofNode::BranchKeyNotFound {
                        left_hash: l_h,
                        right_hash: r_h,
                        left_key: l_s,
                        right_key: r_s,
                    })
                    // proof of exclusion of a key, because root_slice != prefix(searched_slice)
                }
            }
            None => MapProof::Empty,
        }
    }

    /// Returns an iterator over the entries of the map in ascending order. The iterator element
    /// type is (K, V).
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
    /// type is K.
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
    /// element type is V.
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
    /// specified key. The iterator element type is (K, V).
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
            base_iter: self.base.iter_from(&LEAF_KEY_PREFIX, &DBKey::leaf(from)),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the keys of the map in ascending order starting from the
    /// specified key. The iterator element type is K.
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
            base_iter: self.base.iter_from(&LEAF_KEY_PREFIX, &DBKey::leaf(from)),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the values of the map in ascending order of keys starting from the
    /// specified key. The iterator element type is V.
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
        ProofMapIndexValues { base_iter: self.base.iter_from(&LEAF_KEY_PREFIX, &DBKey::leaf(from)) }
    }
}

impl<'a, K, V> ProofMapIndex<&'a mut Fork, K, V>
where
    K: ProofMapKey,
    V: StorageValue,
{
    fn insert_leaf(&mut self, key: &DBKey, value: V) -> Hash {
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
        key_slice: &DBKey,
        value: V,
    ) -> (Option<u16>, Hash) {
        let mut child_slice = parent.child_slice(key_slice.get(0));
        child_slice.set_from(key_slice.from());
        // If the slice is fully fit in key then there is a two cases
        let i = child_slice.common_prefix(key_slice);
        if child_slice.len() == i {
            // check that child is leaf to avoid unnecessary read
            if child_slice.is_leaf() {
                // there is a leaf in branch and we needs to update its value
                let hash = self.insert_leaf(key_slice, value);
                (None, hash)
            } else {
                match self.get_node_unchecked(&child_slice) {
                    Node::Leaf(_) => {
                        unreachable!("Something went wrong!");
                    }
                    // There is a child in branch and we needs to lookup it recursively
                    Node::Branch(mut branch) => {
                        let (j, h) = self.insert_branch(&branch, &key_slice.suffix(i), value);
                        match j {
                            Some(j) => {
                                branch.set_child(
                                    key_slice.get(i),
                                    &key_slice.suffix(i).truncate(j),
                                    &h,
                                )
                            }
                            None => branch.set_child_hash(key_slice.get(i), &h),
                        };
                        let hash = branch.hash();
                        self.base.put(&child_slice, branch);
                        (None, hash)
                    }
                }
            }
        } else {
            // A simple case of inserting a new branch
            let suffix_slice = key_slice.suffix(i);
            let mut new_branch = BranchNode::empty();
            // Add a new leaf
            let hash = self.insert_leaf(&suffix_slice, value);
            new_branch.set_child(suffix_slice.get(0), &suffix_slice, &hash);
            // Move current branch
            new_branch.set_child(
                child_slice.get(i),
                &child_slice.suffix(i),
                parent.child_hash(key_slice.get(0)),
            );

            let hash = new_branch.hash();
            self.base.put(&key_slice.truncate(i), new_branch);
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
        let key_slice = DBKey::leaf(key);
        match self.get_root_node() {
            Some((prefix, Node::Leaf(prefix_data))) => {
                let prefix_slice = prefix;
                let i = prefix_slice.common_prefix(&key_slice);

                let leaf_hash = self.insert_leaf(&key_slice, value);
                if i < key_slice.len() {
                    let mut branch = BranchNode::empty();
                    branch.set_child(key_slice.get(i), &key_slice.suffix(i), &leaf_hash);
                    branch.set_child(
                        prefix_slice.get(i),
                        &prefix_slice.suffix(i),
                        &prefix_data.hash(),
                    );
                    let new_prefix = key_slice.truncate(i);
                    self.base.put(&new_prefix, branch);
                }
            }
            Some((prefix, Node::Branch(mut branch))) => {
                let prefix_slice = prefix;
                let i = prefix_slice.common_prefix(&key_slice);

                if i == prefix_slice.len() {
                    let suffix_slice = key_slice.suffix(i);
                    // Just cut the prefix and recursively descent on.
                    let (j, h) = self.insert_branch(&branch, &suffix_slice, value);
                    match j {
                        Some(j) => {
                            branch.set_child(suffix_slice.get(0), &suffix_slice.truncate(j), &h)
                        }
                        None => branch.set_child_hash(suffix_slice.get(0), &h),
                    };
                    self.base.put(&prefix_slice, branch);
                } else {
                    // Inserts a new branch and adds current branch as its child
                    let hash = self.insert_leaf(&key_slice, value);
                    let mut new_branch = BranchNode::empty();
                    new_branch.set_child(
                        prefix_slice.get(i),
                        &prefix_slice.suffix(i),
                        &branch.hash(),
                    );
                    new_branch.set_child(key_slice.get(i), &key_slice.suffix(i), &hash);
                    // Saves a new branch
                    let new_prefix = prefix_slice.truncate(i);
                    self.base.put(&new_prefix, new_branch);
                }
            }
            None => {
                self.insert_leaf(&key_slice, value);
            }
        }
    }

    fn remove_node(&mut self, parent: &BranchNode, key_slice: &DBKey) -> RemoveResult {
        let mut child_slice = parent.child_slice(key_slice.get(0));
        child_slice.set_from(key_slice.from());
        let i = child_slice.common_prefix(key_slice);

        if i == child_slice.len() {
            match self.get_node_unchecked(&child_slice) {
                Node::Leaf(_) => {
                    self.base.remove(key_slice);
                    return RemoveResult::Leaf;
                }
                Node::Branch(mut branch) => {
                    let suffix_slice = key_slice.suffix(i);
                    match self.remove_node(&branch, &suffix_slice) {
                        RemoveResult::Leaf => {
                            let child = !suffix_slice.get(0);
                            let key = branch.child_slice(child);
                            let hash = branch.child_hash(child);

                            self.base.remove(&child_slice);

                            return RemoveResult::Branch((key, *hash));
                        }
                        RemoveResult::Branch((key, hash)) => {
                            let mut new_child_slice = key.clone();
                            new_child_slice.set_from(suffix_slice.from());

                            branch.set_child(suffix_slice.get(0), &new_child_slice, &hash);
                            let h = branch.hash();
                            self.base.put(&child_slice, branch);
                            return RemoveResult::UpdateHash(h);
                        }
                        RemoveResult::UpdateHash(hash) => {
                            branch.set_child_hash(suffix_slice.get(0), &hash);
                            let h = branch.hash();
                            self.base.put(&child_slice, branch);
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
        let key_slice = DBKey::leaf(key);
        match self.get_root_node() {
            // If we have only on leaf, then we just need to remove it (if any)
            Some((prefix, Node::Leaf(_))) => {
                let key = key_slice;
                if key == prefix {
                    self.base.remove(&key);
                }
            }
            Some((prefix, Node::Branch(mut branch))) => {
                // Truncate prefix
                let i = prefix.common_prefix(&key_slice);
                if i == prefix.len() {
                    let suffix_slice = key_slice.suffix(i);
                    match self.remove_node(&branch, &suffix_slice) {
                        RemoveResult::Leaf => self.base.remove(&prefix),
                        RemoveResult::Branch((key, hash)) => {
                            let mut new_child_slice = key.clone();
                            new_child_slice.set_from(suffix_slice.from());
                            branch.set_child(suffix_slice.get(0), &new_child_slice, &hash);
                            self.base.put(&prefix, branch);
                        }
                        RemoveResult::UpdateHash(hash) => {
                            branch.set_child_hash(suffix_slice.get(0), &hash);
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

impl<'a, T, K, V> ::std::iter::IntoIterator for &'a ProofMapIndex<T, K, V>
where
    T: AsRef<Snapshot>,
    K: ProofMapKey,
    V: StorageValue,
{
    type Item = (K, V);
    type IntoIter = ProofMapIndexIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> Iterator for ProofMapIndexIter<'a, K, V>
where
    K: ProofMapKey,
    V: StorageValue,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, v)| (K::read(k.as_ref()), v))
    }
}


impl<'a, K> Iterator for ProofMapIndexKeys<'a, K>
where
    K: ProofMapKey,
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, _)| K::read(k.as_ref()))
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
