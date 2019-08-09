// Copyright 2019 The Exonum Team
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

#[doc(hidden)]
pub use self::node::{BranchNode, Node};
pub use self::{
    key::{ProofPath, KEY_SIZE as PROOF_MAP_KEY_SIZE, PROOF_PATH_SIZE},
    proof::{CheckedMapProof, MapProof, MapProofError},
};

use std::{
    fmt,
    io::{Read, Write},
    marker::PhantomData,
};

use exonum_crypto::Hash;

use self::{
    key::{BitsRange, ChildKind, VALUE_KEY_PREFIX},
    proof::{create_multiproof, create_proof},
};
use crate::views::{AnyObject, IndexAddress};
use crate::{
    views::{
        BinaryAttribute, IndexAccess, IndexBuilder, IndexState, IndexType, Iter as ViewIter, View,
    },
    BinaryKey, BinaryValue, HashTag, ObjectHash,
};

mod key;
mod node;
mod proof;
#[cfg(test)]
mod tests;

/// A Merkelized version of a map that provides proofs of existence or non-existence for the map
/// keys.
///
/// `ProofMapIndex` implements a Merkle Patricia tree, storing values as leaves.
/// `ProofMapIndex` requires that keys implement the [`BinaryKey`] trait and
/// values implement the [`BinaryValue`] trait.
///
/// [`BinaryKey`]: ../trait.BinaryKey.html
/// [`BinaryValue`]: ../trait.BinaryValue.html
pub struct ProofMapIndex<T: IndexAccess, K, V> {
    base: View<T>,
    state: IndexState<T, Option<ProofPath>>,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

/// An iterator over the entries of a `ProofMapIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] method on [`ProofMapIndex`]. See its documentation for details.
///
/// [`iter`]: struct.ProofMapIndex.html#method.iter
/// [`iter_from`]: struct.ProofMapIndex.html#method.iter_from
/// [`ProofMapIndex`]: struct.ProofMapIndex.html
#[derive(Debug)]
pub struct ProofMapIndexIter<'a, K, V> {
    base_iter: ViewIter<'a, Vec<u8>, V>,
    _k: PhantomData<K>,
}

/// An iterator over the keys of a `ProofMapIndex`.
///
/// This struct is created by the [`keys`] or
/// [`keys_from`] method on [`ProofMapIndex`]. See its documentation for details.
///
/// [`keys`]: struct.ProofMapIndex.html#method.keys
/// [`keys_from`]: struct.ProofMapIndex.html#method.keys_from
/// [`ProofMapIndex`]: struct.ProofMapIndex.html
#[derive(Debug)]
pub struct ProofMapIndexKeys<'a, K> {
    base_iter: ViewIter<'a, Vec<u8>, ()>,
    _k: PhantomData<K>,
}

/// An iterator over the values of a `ProofMapIndex`.
///
/// This struct is created by the [`values`] or
/// [`values_from`] method on [`ProofMapIndex`]. See its documentation for details.
///
/// [`values`]: struct.ProofMapIndex.html#method.values
/// [`values_from`]: struct.ProofMapIndex.html#method.values_from
/// [`ProofMapIndex`]: struct.ProofMapIndex.html
#[derive(Debug)]
pub struct ProofMapIndexValues<'a, V> {
    base_iter: ViewIter<'a, Vec<u8>, V>,
}

impl<T, K, V> AnyObject<T> for ProofMapIndex<T, K, V>
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash,
    V: BinaryValue + ObjectHash,
{
    fn view(self) -> View<T> {
        self.base
    }

    fn object_type(&self) -> IndexType {
        IndexType::ProofMap
    }

    fn metadata(&self) -> Vec<u8> {
        self.state.metadata().to_bytes()
    }
}

/// TODO Clarify documentation. [ECR-2820]
enum RemoveAction {
    KeyNotFound,
    Leaf,
    Branch((ProofPath, Hash)),
    UpdateHash(Hash),
}

/// The internal key representation that uses to address values.
///
/// Represents the original key bytes with the `VALUE_KEY_PREFIX` prefix.
/// TODO Clarify documentation. [ECR-2820]
trait ValuePath: ToOwned {
    /// Converts the given key to the value path bytes.
    fn to_value_path(&self) -> Vec<u8>;
    /// Extracts the given key from the value path bytes.
    fn from_value_path(bytes: &[u8]) -> Self::Owned;
}

impl<T: BinaryKey> ValuePath for T {
    fn to_value_path(&self) -> Vec<u8> {
        let mut buf = vec![0_u8; self.size() + 1];
        buf[0] = VALUE_KEY_PREFIX;
        self.write(&mut buf[1..]);
        buf
    }

    fn from_value_path(buffer: &[u8]) -> Self::Owned {
        Self::read(&buffer[1..])
    }
}

impl BinaryAttribute for Option<ProofPath> {
    fn size(&self) -> usize {
        match self {
            Some(path) => path.size(),
            None => 0,
        }
    }

    fn write<W: Write>(&self, buffer: &mut W) {
        if let Some(path) = self {
            let mut tmp = [0_u8; PROOF_PATH_SIZE];
            path.write(&mut tmp);
            buffer.write_all(&tmp).unwrap();
        }
    }

    fn read<R: Read>(buffer: &mut R) -> Self {
        let mut tmp = [0_u8; PROOF_PATH_SIZE];
        match buffer.read(&mut tmp).unwrap() {
            0 => None,
            PROOF_PATH_SIZE => Some(ProofPath::read(&tmp)),
            other => panic!("Unexpected attribute length: {}", other),
        }
    }
}

impl<T, K, V> ProofMapIndex<T, K, V>
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash,
    V: BinaryValue + ObjectHash,
{
    /// Creates a new index representation based on the name and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case, only
    /// immutable methods are available. In the second case, both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &snapshot);
    ///
    /// let fork = db.fork();
    /// let mut mut_index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new(name, &fork);
    /// ```
    pub fn new<S: Into<String>>(index_name: S, view: T) -> Self {
        let (base, state) = IndexBuilder::new(view)
            .index_type(IndexType::ProofMap)
            .index_name(index_name)
            .build();
        Self {
            base,
            state,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Creates a new index representation based on the name, common prefix of its keys
    /// and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case, only
    /// immutable methods are available. In the second case, both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
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
    /// let fork = db.fork();
    /// let mut mut_index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new_in_family(
    ///     name,
    ///     &index_id,
    ///     &fork,
    ///  );
    /// ```
    pub fn new_in_family<S, I>(family_name: S, index_id: &I, view: T) -> Self
    where
        I: BinaryKey,
        I: ?Sized,
        S: Into<String>,
    {
        let (base, state) = IndexBuilder::new(view)
            .index_type(IndexType::ProofMap)
            .index_name(family_name)
            .family_id(index_id)
            .build();
        Self {
            base,
            state,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    pub fn get_from<I: Into<IndexAddress>>(address: I, access: T) -> Option<Self> {
        IndexBuilder::from_address(address, access)
            .index_type(IndexType::ProofMap)
            .build_existed()
            .map(|(base, state)| Self {
                base,
                state,
                _k: PhantomData,
                _v: PhantomData,
            })
    }

    pub fn create_from<I: Into<IndexAddress>>(address: I, access: T) -> Self {
        let (base, state) = IndexBuilder::from_address(address, access)
            .index_type(IndexType::ProofMap)
            .build();

        Self {
            base,
            state,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    fn get_root_path(&self) -> Option<ProofPath> {
        self.state.get()
    }

    fn get_root_node(&self) -> Option<(ProofPath, Node)> {
        self.get_root_path().map(|key| {
            let node = self.get_node_unchecked(&key);
            (key, node)
        })
    }

    fn get_node_unchecked(&self, key: &ProofPath) -> Node {
        // TODO: Unwraps? (ECR-84)
        if key.is_leaf() {
            Node::Leaf(self.base.get(key).unwrap())
        } else {
            Node::Branch(self.base.get(key).unwrap())
        }
    }

    fn get_value_unchecked(&self, key: &K) -> V {
        self.get(key).expect("Value for the given key is absent")
    }

    pub(crate) fn merkle_root(&self) -> Hash {
        match self.get_root_node() {
            Some((path, Node::Leaf(hash))) => HashTag::hash_single_entry_map(&path, &hash),
            Some((_, Node::Branch(branch))) => branch.object_hash(),
            None => Hash::zero(),
        }
    }

    /// Returns a value corresponding to the key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &fork);
    ///
    /// let hash = Hash::default();
    /// assert_eq!(None, index.get(&hash));
    ///
    /// index.put(&hash, 2);
    /// assert_eq!(Some(2), index.get(&hash));
    /// ```
    pub fn get(&self, key: &K) -> Option<V> {
        self.base.get(&key.to_value_path())
    }

    /// Returns `true` if the map contains a value for the specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &fork);
    ///
    /// let hash = Hash::default();
    /// assert!(!index.contains(&hash));
    ///
    /// index.put(&hash, 2);
    /// assert!(index.contains(&hash));
    /// ```
    pub fn contains(&self, key: &K) -> bool {
        self.base.contains(&key.to_value_path())
    }

    /// Returns the proof of existence or non-existence for the specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Hash, u8> = ProofMapIndex::new("index", &snapshot);
    ///
    /// let proof = index.get_proof(Hash::default());
    /// ```
    pub fn get_proof(&self, key: K) -> MapProof<K, V> {
        create_proof(
            key,
            self.get_root_node(),
            |path| self.get_node_unchecked(path),
            |key| self.get_value_unchecked(key),
        )
    }

    /// Returns the combined proof of existence or non-existence for the multiple specified keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: ProofMapIndex<_, Vec<u8>, u8> = ProofMapIndex::new("index", &snapshot);
    ///
    /// let proof = index.get_multiproof(vec![vec![0; 32], vec![1; 32]]);
    /// ```
    pub fn get_multiproof<KI>(&self, keys: KI) -> MapProof<K, V>
    where
        KI: IntoIterator<Item = K>,
    {
        create_multiproof(
            keys,
            self.get_root_node(),
            |path| self.get_node_unchecked(path),
            |key| self.get_value_unchecked(key),
        )
    }

    /// Returns an iterator over the entries of the map in ascending order. The iterator element
    /// type is `(K::Output, V)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
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
            base_iter: self.base.iter(&VALUE_KEY_PREFIX),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the keys of the map in ascending order. The iterator element
    /// type is `K::Output`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
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
            base_iter: self.base.iter(&VALUE_KEY_PREFIX),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the values of the map in ascending order of keys. The iterator
    /// element type is `V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
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
            base_iter: self.base.iter(&VALUE_KEY_PREFIX),
        }
    }

    /// Returns an iterator over the entries of the map in ascending order starting from the
    /// specified key. The iterator element type is `(K::Output, V)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
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
            base_iter: self
                .base
                .iter_from(&VALUE_KEY_PREFIX, &from.to_value_path()),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the keys of the map in ascending order starting from the
    /// specified key. The iterator element type is `K::Output`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
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
            base_iter: self
                .base
                .iter_from(&VALUE_KEY_PREFIX, &from.to_value_path()),
            _k: PhantomData,
        }
    }

    /// Returns an iterator over the values of the map in ascending order of keys starting from the
    /// specified key. The iterator element type is `V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
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
            base_iter: self
                .base
                .iter_from(&VALUE_KEY_PREFIX, &from.to_value_path()),
        }
    }

    fn insert_leaf(&mut self, proof_path: &ProofPath, key: &K, value: V) -> Hash {
        debug_assert!(proof_path.is_leaf());
        let hash = HashTag::hash_leaf(&value.to_bytes());
        self.base.put(proof_path, hash);
        self.base.put(&key.to_value_path(), value);
        hash
    }

    fn remove_leaf(&mut self, proof_path: &ProofPath, key: &K) {
        self.base.remove(proof_path);
        self.base.remove(&key.to_value_path());
    }

    fn update_root_path(&mut self, path: Option<ProofPath>) {
        self.state.set(path)
    }

    // Inserts a new node of the current branch and returns the updated hash
    // or, if a new node has a shorter key, returns a new key length.
    fn insert_branch(
        &mut self,
        parent: &BranchNode,
        proof_path: &ProofPath,
        key: &K,
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
                let hash = self.insert_leaf(proof_path, key, value);
                (None, hash)
            } else {
                match self.get_node_unchecked(&child_path) {
                    Node::Leaf(_) => {
                        unreachable!("Something went wrong!");
                    }
                    // There is a child in branch and we needs to lookup it recursively
                    Node::Branch(mut branch) => {
                        let (j, h) = self.insert_branch(&branch, &proof_path.suffix(i), key, value);
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
                        let hash = branch.object_hash();
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
            let hash = self.insert_leaf(&suffix_path, key, value);
            new_branch.set_child(suffix_path.bit(0), &suffix_path, &hash);
            // Move current branch
            new_branch.set_child(
                child_path.bit(i),
                &child_path.suffix(i),
                &parent.child_hash(proof_path.bit(0)),
            );

            let hash = new_branch.object_hash();
            self.base.put(&proof_path.prefix(i), new_branch);
            (Some(i), hash)
        }
    }

    fn remove_node(
        &mut self,
        parent: &BranchNode,
        proof_path: &ProofPath,
        key: &K,
    ) -> RemoveAction {
        let child_path = parent
            .child_path(proof_path.bit(0))
            .start_from(proof_path.start());
        let i = child_path.common_prefix_len(proof_path);

        if i == child_path.len() {
            match self.get_node_unchecked(&child_path) {
                Node::Leaf(_) => {
                    self.remove_leaf(proof_path, key);
                    return RemoveAction::Leaf;
                }
                Node::Branch(mut branch) => {
                    let suffix_path = proof_path.suffix(i);
                    match self.remove_node(&branch, &suffix_path, key) {
                        RemoveAction::Leaf => {
                            let child = !suffix_path.bit(0);
                            let key = branch.child_path(child);
                            let hash = branch.child_hash(child);

                            self.base.remove(&child_path);
                            return RemoveAction::Branch((key, hash));
                        }
                        RemoveAction::Branch((key, hash)) => {
                            let new_child_path = key.start_from(suffix_path.start());
                            branch.set_child(suffix_path.bit(0), &new_child_path, &hash);
                            let h = branch.object_hash();

                            self.base.put(&child_path, branch);
                            return RemoveAction::UpdateHash(h);
                        }
                        RemoveAction::UpdateHash(hash) => {
                            branch.set_child_hash(suffix_path.bit(0), &hash);
                            let h = branch.object_hash();

                            self.base.put(&child_path, branch);
                            return RemoveAction::UpdateHash(h);
                        }
                        RemoveAction::KeyNotFound => return RemoveAction::KeyNotFound,
                    }
                }
            }
        }
        RemoveAction::KeyNotFound
    }

    /// Inserts the key-value pair into the proof map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &fork);
    ///
    /// let hash = Hash::default();
    /// index.put(&hash, 2);
    /// assert!(index.contains(&hash));
    /// ```
    pub fn put(&mut self, key: &K, value: V) {
        let proof_path = ProofPath::new(key);
        let root_path = match self.get_root_node() {
            Some((prefix, Node::Leaf(prefix_data))) => {
                let prefix_path = prefix;
                let i = prefix_path.common_prefix_len(&proof_path);

                let leaf_hash = self.insert_leaf(&proof_path, key, value);
                if i < proof_path.len() {
                    let mut branch = BranchNode::empty();
                    branch.set_child(proof_path.bit(i), &proof_path.suffix(i), &leaf_hash);
                    branch.set_child(
                        prefix_path.bit(i),
                        &prefix_path.suffix(i),
                        &prefix_data.object_hash(),
                    );
                    let new_prefix = proof_path.prefix(i);
                    self.base.put(&new_prefix, branch);
                    new_prefix
                } else {
                    proof_path
                }
            }
            Some((prefix, Node::Branch(mut branch))) => {
                let prefix_path = prefix;
                let i = prefix_path.common_prefix_len(&proof_path);

                if i == prefix_path.len() {
                    let suffix_path = proof_path.suffix(i);
                    // Just cut the prefix and recursively descent on.
                    let (j, h) = self.insert_branch(&branch, &suffix_path, key, value);
                    match j {
                        Some(j) => branch.set_child(suffix_path.bit(0), &suffix_path.prefix(j), &h),
                        None => branch.set_child_hash(suffix_path.bit(0), &h),
                    };
                    self.base.put(&prefix_path, branch);
                    prefix_path
                } else {
                    // Inserts a new branch and adds current branch as its child
                    let hash = self.insert_leaf(&proof_path, key, value);
                    let mut new_branch = BranchNode::empty();
                    new_branch.set_child(
                        prefix_path.bit(i),
                        &prefix_path.suffix(i),
                        &branch.object_hash(),
                    );
                    new_branch.set_child(proof_path.bit(i), &proof_path.suffix(i), &hash);
                    // Saves a new branch
                    let new_prefix = prefix_path.prefix(i);
                    self.base.put(&new_prefix, new_branch);
                    new_prefix
                }
            }
            None => {
                self.insert_leaf(&proof_path, key, value);
                proof_path
            }
        };
        self.update_root_path(Some(root_path));
    }

    /// Removes a key from the proof map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &fork);
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
                if proof_path == prefix {
                    self.remove_leaf(&proof_path, key);
                    self.update_root_path(None);
                }
            }
            Some((prefix, Node::Branch(mut branch))) => {
                // Truncate prefix
                let i = prefix.common_prefix_len(&proof_path);
                if i == prefix.len() {
                    let suffix_path = proof_path.suffix(i);
                    match self.remove_node(&branch, &suffix_path, key) {
                        RemoveAction::Leaf => {
                            // After removing one of leaves second child becomes a new root.
                            self.base.remove(&prefix);
                            self.update_root_path(Some(branch.child_path(!suffix_path.bit(0))));
                        }
                        RemoveAction::Branch((key, hash)) => {
                            let new_child_path = key.start_from(suffix_path.start());
                            branch.set_child(suffix_path.bit(0), &new_child_path, &hash);
                            self.base.put(&prefix, branch);
                        }
                        RemoveAction::UpdateHash(hash) => {
                            branch.set_child_hash(suffix_path.bit(0), &hash);
                            self.base.put(&prefix, branch);
                        }
                        RemoveAction::KeyNotFound => return,
                    }
                } else {
                    return;
                }
            }
            None => return,
        }
    }

    /// Clears the proof map, removing all entries.
    ///
    /// # Notes
    ///
    /// Currently, this method is not optimized to delete a large set of data. During the execution of
    /// this method, the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &fork);
    ///
    /// let hash = Hash::default();
    /// index.put(&hash, 2);
    /// assert!(index.contains(&hash));
    ///
    /// index.clear();
    /// assert!(!index.contains(&hash));
    /// ```
    pub fn clear(&mut self) {
        self.base.clear();
        self.state.clear();
    }
}

impl<T, K, V> ObjectHash for ProofMapIndex<T, K, V>
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash,
    V: BinaryValue + ObjectHash,
{
    /// Returns the hash of the proof map object. See [`HashTag::hash_map_node`].
    /// For hash of the empty map see [`HashTag::empty_map_hash`].
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofMapIndex, HashTag, ObjectHash};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofMapIndex::new(name, &fork);
    ///
    /// let default_hash = index.object_hash();
    /// assert_eq!(HashTag::empty_map_hash(), default_hash);
    ///
    /// index.put(&default_hash, 100);
    /// let hash = index.object_hash();
    /// assert_ne!(hash, default_hash);
    /// ```
    ///
    /// [`HashTag::hash_map_node`]: ../enum.HashTag.html#method.hash_map_node
    /// [`HashTag::empty_map_hash`]: ../enum.HashTag.html#method.empty_map_hash
    fn object_hash(&self) -> Hash {
        HashTag::hash_map_node(self.merkle_root())
    }
}

impl<'a, T, K, V> ::std::iter::IntoIterator for &'a ProofMapIndex<T, K, V>
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash,
    V: BinaryValue + ObjectHash,
{
    type Item = (K::Owned, V);
    type IntoIter = ProofMapIndexIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> Iterator for ProofMapIndexIter<'a, K, V>
where
    K: BinaryKey,
    V: BinaryValue,
{
    type Item = (K::Owned, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter
            .next()
            .map(|(k, v)| (K::from_value_path(&k), v))
    }
}

impl<'a, K> Iterator for ProofMapIndexKeys<'a, K>
where
    K: BinaryKey,
{
    type Item = K::Owned;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, _)| K::from_value_path(&k))
    }
}

impl<'a, V> Iterator for ProofMapIndexValues<'a, V>
where
    V: BinaryValue,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(_, v)| v)
    }
}

#[allow(clippy::use_self)]
impl<T, K, V> fmt::Debug for ProofMapIndex<T, K, V>
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash,
    V: BinaryValue + ObjectHash + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct Entry<'a, T: 'a + IndexAccess, K: 'a, V: 'a + BinaryValue> {
            index: &'a ProofMapIndex<T, K, V>,
            path: ProofPath,
            hash: Hash,
            node: Node,
        }

        impl<'a, T, K, V> Entry<'a, T, K, V>
        where
            T: IndexAccess,
            K: BinaryKey + ObjectHash,
            V: BinaryValue + ObjectHash,
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
                    self_branch.child_hash(kind),
                    self_branch.child_path(kind),
                )
            }
        }

        impl<'a, T, K, V> fmt::Debug for Entry<'a, T, K, V>
        where
            T: IndexAccess,
            K: BinaryKey + ObjectHash,
            V: BinaryValue + ObjectHash + fmt::Debug,
        {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self.node {
                    Node::Leaf(ref value) => f
                        .debug_struct("Leaf")
                        .field("key", &self.path)
                        .field("hash", &self.hash)
                        .field("value", value)
                        .finish(),
                    Node::Branch(ref branch) => f
                        .debug_struct("Branch")
                        .field("path", &self.path)
                        .field("hash", &self.hash)
                        .field("left", &self.child(branch, ChildKind::Left))
                        .field("right", &self.child(branch, ChildKind::Right))
                        .finish(),
                }
            }
        }

        if let Some(prefix) = self.get_root_path() {
            let root_entry = Entry::new(self, self.object_hash(), prefix);
            f.debug_struct("ProofMapIndex")
                .field("entries", &root_entry)
                .finish()
        } else {
            f.debug_struct("ProofMapIndex").finish()
        }
    }
}
