// Copyright 2018 The Exonum Team
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

pub use self::{
    key::{HashedKey, ProofMapKey, ProofPath, KEY_SIZE as PROOF_MAP_KEY_SIZE},
    proof::{CheckedMapProof, MapProof, MapProofError},
};

use std::{fmt, marker::PhantomData};

use self::{
    key::{BitsRange, ChildKind, LEAF_KEY_PREFIX}, node::{BranchNode, Node},
    proof::{create_multiproof, create_proof},
};
use super::{
    base_index::{BaseIndex, BaseIndexIter}, indexes_metadata::IndexType, Fork, Snapshot,
    StorageKey, StorageValue,
};
use crypto::{CryptoHash, Hash, HashStream};

mod key;
mod node;
mod proof;
#[cfg(test)]
mod tests;

/// A Merkelized version of a map that provides proofs of existence or non-existence for the map
/// keys.
///
/// `ProofMapIndex` implements a Merkle Patricia tree, storing the values as leaves.
/// `ProofMapIndex` requires that the keys implement [`ProofMapKey`] and values implement the
/// [`StorageValue`] trait.
///
/// **The size of the proof map keys must be exactly 32 bytes and the keys must have a uniform
/// distribution.** Usually [`Hash`] and [`PublicKey`] are used as types of proof map keys.
///
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

impl<T, K, V> ProofMapIndex<T, K, V>
where
    T: AsRef<Snapshot>,
    K: ProofMapKey,
    V: StorageValue,
{
    /// Creates a new index representation based on the name and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    ///
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
    /// ```
    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {
        ProofMapIndex {
            base: BaseIndex::new(index_name, IndexType::ProofMap, view),
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
    ///
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
    /// let index_id = vec![01];
    ///
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new_in_family(
    ///     name,
    ///     &index_id,
    ///     &snapshot,
    ///  );
    ///
    /// let mut fork = db.fork();
    /// let mut mut_index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new_in_family(
    ///     name,
    ///     &index_id,
    ///     &mut fork,
    ///  );
    /// ```
    pub fn new_in_family<S: AsRef<str>, I: StorageKey>(
        family_name: S,
        index_id: &I,
        view: T,
    ) -> Self {
        ProofMapIndex {
            base: BaseIndex::new_in_family(family_name, index_id, IndexType::ProofMap, view),
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    fn get_root_path(&self) -> Option<ProofPath> {
        self.base
            .iter::<_, ProofPath, _>(&())
            .next()
            .map(|(k, _): (ProofPath, ())| k)
    }

    fn get_root_node(&self) -> Option<(ProofPath, Node<V>)> {
        self.get_root_path().map(|key| {
            let node = self.get_node_unchecked(&key);
            (key, node)
        })
    }

    fn get_node_unchecked(&self, key: &ProofPath) -> Node<V> {
        // TODO: Unwraps? (ECR-84)
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
    /// let default_hash = index.merkle_root();
    /// assert_eq!(Hash::default(), default_hash);
    ///
    /// index.put(&default_hash, 100);
    /// let hash = index.merkle_root();
    /// assert_ne!(hash, default_hash);
    /// ```
    pub fn merkle_root(&self) -> Hash {
        match self.get_root_node() {
            Some((k, Node::Leaf(v))) => HashStream::new()
                .update(k.as_bytes())
                .update(v.hash().as_ref())
                .hash(),
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
    /// ```
    pub fn get_proof(&self, key: K) -> MapProof<K, V> {
        create_proof(key, self.get_root_node(), |path| {
            self.get_node_unchecked(path)
        })
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
    /// ```
    pub fn get_multiproof<KI>(&self, keys: KI) -> MapProof<K, V>
    where
        KI: IntoIterator<Item = K>,
    {
        create_multiproof(keys, self.get_root_node(), |path| {
            self.get_node_unchecked(path)
        })
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
        ProofMapIndexValues {
            base_iter: self.base.iter(&LEAF_KEY_PREFIX),
        }
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
        let child_path = parent
            .child_path(proof_path.bit(0))
            .start_from(proof_path.start());
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
        let child_path = parent
            .child_path(proof_path.bit(0))
            .start_from(proof_path.start());
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

impl<'a, T, K, V> ::std::iter::IntoIterator for &'a ProofMapIndex<T, K, V>
where
    T: AsRef<Snapshot>,
    K: ProofMapKey,
    V: StorageValue,
{
    type Item = (K::Output, V);
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
    type Item = (K::Output, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter
            .next()
            .map(|(k, v)| (K::read_key(k.raw_key()), v))
    }
}

impl<'a, K> Iterator for ProofMapIndexKeys<'a, K>
where
    K: ProofMapKey,
{
    type Item = K::Output;

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
                    Node::Leaf(ref value) => f.debug_struct("Leaf")
                        .field("key", &self.path)
                        .field("hash", &self.hash)
                        .field("value", value)
                        .finish(),
                    Node::Branch(ref branch) => f.debug_struct("Branch")
                        .field("path", &self.path)
                        .field("hash", &self.hash)
                        .field("left", &self.child(branch, ChildKind::Left))
                        .field("right", &self.child(branch, ChildKind::Right))
                        .finish(),
                }
            }
        }

        if let Some(prefix) = self.get_root_path() {
            let root_entry = Entry::new(self, self.merkle_root(), prefix);
            f.debug_struct("ProofMapIndex")
                .field("entries", &root_entry)
                .finish()
        } else {
            f.debug_struct("ProofMapIndex").finish()
        }
    }
}
