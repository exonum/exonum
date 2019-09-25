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

//! An implementation of a Merkelized version of an array list (Merkle tree).

pub use self::proof::{CheckedListProof, ListProof, ListProofError, ValidationError};

use std::{cmp, iter, marker::PhantomData, ops::RangeBounds};

use exonum_crypto::Hash;

use self::{
    key::ProofListKey,
    proof_builder::{BuildProof, MerkleTree},
};
use crate::views::IndexAddress;
use crate::{
    hash::HashTag,
    views::{AnyObject, IndexAccess, IndexBuilder, IndexState, IndexType, Iter as ViewIter, View},
    BinaryKey, BinaryValue, ObjectHash,
};

mod key;
mod proof;
mod proof_builder;
#[cfg(test)]
mod tests;

// TODO: Implement pop and truncate methods for Merkle tree. (ECR-173)

fn tree_height_by_length(len: u64) -> u8 {
    if len == 0 {
        0
    } else {
        len.next_power_of_two().trailing_zeros() as u8 + 1
    }
}

/// A Merkelized version of an array list that provides proofs of existence for the list items.
///
/// `ProofListIndex` implements a Merkle tree, storing elements as leaves and using `u64` as
/// an index. `ProofListIndex` requires that elements implement the [`BinaryValue`] trait.
///
/// [`BinaryValue`]: ../trait.BinaryValue.html
#[derive(Debug)]
pub struct ProofListIndex<T: IndexAccess, V> {
    base: View<T>,
    state: IndexState<T, u64>,
    _v: PhantomData<V>,
}

/// An iterator over the items of a `ProofListIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] method on [`ProofListIndex`]. See its documentation for details.
///
/// [`iter`]: struct.ProofListIndex.html#method.iter
/// [`iter_from`]: struct.ProofListIndex.html#method.iter_from
/// [`ProofListIndex`]: struct.ProofListIndex.html
#[derive(Debug)]
pub struct ProofListIndexIter<'a, V> {
    base_iter: ViewIter<'a, ProofListKey, V>,
}

impl<T, V> AnyObject<T> for ProofListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn view(self) -> View<T> {
        self.base
    }

    fn object_type(&self) -> IndexType {
        IndexType::ProofList
    }

    fn metadata(&self) -> Vec<u8> {
        self.state.metadata().to_bytes()
    }
}

impl<T, V> MerkleTree<V> for ProofListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn len(&self) -> u64 {
        self.len()
    }

    fn node(&self, position: ProofListKey) -> Hash {
        self.get_branch_unchecked(position)
    }

    fn merkle_root(&self) -> Hash {
        self.get_branch(self.root_key()).unwrap_or_default()
    }

    fn values<'s>(&'s self, start_index: u64) -> Box<dyn Iterator<Item = V> + 's> {
        Box::new(self.iter_from(start_index))
    }
}

impl<T, V> ProofListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
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
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    ///
    /// let snapshot = db.snapshot();
    /// let index: ProofListIndex<_, u8> = ProofListIndex::new(name, &snapshot);
    ///
    /// let fork = db.fork();
    /// let mut mut_index: ProofListIndex<_, u8> = ProofListIndex::new(name, &fork);
    /// ```
    pub fn new<S: Into<String>>(index_name: S, index_access: T) -> Self {
        let (base, state) = IndexBuilder::new(index_access)
            .index_type(IndexType::ProofList)
            .index_name(index_name)
            .build();
        Self {
            base,
            state,
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
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let index_id = vec![01];
    ///
    /// let snapshot = db.snapshot();
    /// let index: ProofListIndex<_, u8> =
    ///     ProofListIndex::new_in_family(name, &index_id, &snapshot);
    ///
    /// let fork = db.fork();
    /// let mut mut_index : ProofListIndex<_, u8> =
    ///     ProofListIndex::new_in_family(name, &index_id, &fork);
    /// ```
    pub fn new_in_family<S, I>(family_name: S, index_id: &I, index_access: T) -> Self
    where
        I: BinaryKey,
        I: ?Sized,
        S: Into<String>,
    {
        let (base, state) = IndexBuilder::new(index_access)
            .index_type(IndexType::ProofList)
            .index_name(family_name)
            .family_id(index_id)
            .build();
        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    pub(crate) fn create_from<I: Into<IndexAddress>>(address: I, access: T) -> Self {
        let (base, state) = IndexBuilder::from_address(address, access)
            .index_type(IndexType::ProofList)
            .build();

        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    pub(crate) fn get_from<I: Into<IndexAddress>>(address: I, access: T) -> Option<Self> {
        IndexBuilder::from_address(address, access)
            .index_type(IndexType::ProofList)
            .build_existed()
            .map(|(base, state)| Self {
                base,
                state,
                _v: PhantomData,
            })
    }

    fn has_branch(&self, key: ProofListKey) -> bool {
        key.first_left_leaf_index() < self.len()
    }

    fn get_branch(&self, key: ProofListKey) -> Option<Hash> {
        if self.has_branch(key) {
            self.base.get(&key)
        } else {
            None
        }
    }

    fn get_branch_unchecked(&self, key: ProofListKey) -> Hash {
        debug_assert!(self.has_branch(key));
        self.base.get(&key).unwrap()
    }

    fn root_key(&self) -> ProofListKey {
        ProofListKey::new(self.height(), 0)
    }

    fn set_len(&mut self, len: u64) {
        self.state.set(len)
    }

    /// Returns the element at the indicated position or `None` if the indicated position
    /// is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    /// assert_eq!(None, index.get(0));
    ///
    /// index.push(10);
    /// assert_eq!(Some(10), index.get(0));
    /// ```
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&ProofListKey::leaf(index))
    }

    /// Returns the last element of the proof list or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    /// assert_eq!(None, index.last());
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.last());
    /// ```
    pub fn last(&self) -> Option<V> {
        match self.len() {
            0 => None,
            l => self.get(l - 1),
        }
    }

    /// Returns `true` if the proof list contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    /// assert!(index.is_empty());
    ///
    /// index.push(10);
    /// assert!(!index.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of elements in the proof list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    /// assert_eq!(0, index.len());
    ///
    /// index.push(1);
    /// assert_eq!(1, index.len());
    /// ```
    pub fn len(&self) -> u64 {
        self.state.get()
    }

    /// Returns the height of the Merkle tree built based on the list.
    ///
    /// The height of the empty list is 0; otherwise, the height is computed as `ceil(log2(l)) + 1`,
    /// where `l` is the list length.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    /// assert_eq!(0, index.height());
    /// index.push(1);
    /// assert_eq!(1, index.height());
    /// index.push(1);
    /// assert_eq!(2, index.height());
    /// ```
    pub fn height(&self) -> u8 {
        tree_height_by_length(self.len())
    }

    /// Returns a proof of existence for the list element at the specified position.
    ///
    /// Returns a proof of absence if the list doesn't contain an element with the specified `index`.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    ///
    /// index.push(1);
    /// let proof = index.get_proof(0);
    /// let proof_of_absence = index.get_proof(1);
    /// ```
    pub fn get_proof(&self, index: u64) -> ListProof<V> {
        self.create_proof(index)
    }

    /// Returns the proof of existence for the list elements in the specified range.
    ///
    /// Returns a proof of absence for a range of values, if either or both its bounds
    /// exceed the list state.
    ///
    /// # Panics
    ///
    /// Panics if the range bounds are illegal.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    /// index.extend(vec![1, 2, 3, 4, 5]);
    ///
    /// let range_proof = index.get_range_proof(1..3);
    /// assert!(range_proof.indexes_unchecked().eq(vec![1, 2]));
    /// // This proof will contain only 4 elements with indexes 1..5.
    /// let intersection_proof = index.get_range_proof(1..10);
    /// assert!(intersection_proof.indexes_unchecked().eq(1..5));
    /// // This proof does not contain any elements at all.
    /// let empty_proof = index.get_range_proof(100..10000);
    /// assert!(empty_proof.entries_unchecked().is_empty());
    /// ```
    pub fn get_range_proof<R: RangeBounds<u64>>(&self, range: R) -> ListProof<V> {
        self.create_range_proof(range)
    }

    /// Returns an iterator over the list. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofListIndex<_, u8> = ProofListIndex::new(name, &snapshot);
    ///
    /// for val in index.iter() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter(&self) -> ProofListIndexIter<V> {
        ProofListIndexIter {
            base_iter: self.base.iter(&0_u8),
        }
    }

    /// Returns an iterator over the list starting from the specified position. The iterator
    /// element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ProofListIndex<_, u8> = ProofListIndex::new(name, &snapshot);
    ///
    /// for val in index.iter_from(1) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: u64) -> ProofListIndexIter<V> {
        ProofListIndexIter {
            base_iter: self.base.iter_from(&0_u8, &ProofListKey::leaf(from)),
        }
    }

    /// Updates levels of the tree with heights `2..` after the values in the range
    /// `[first_index, last_index]` were updated.
    ///
    /// # Invariants
    ///
    /// - `self.len()` / `self.height()` is assumed to be correctly set.
    /// - Value hashes (i.e., tree branches on level 1) are assumed to be updated.
    fn update_range(&mut self, mut first_index: u64, mut last_index: u64) {
        let mut last_level_index = self.len() - 1;
        for height in 1..self.height() {
            // Check consistency of the index range.
            debug_assert!(first_index <= last_index);
            // Check consistency with the level length.
            debug_assert!(last_index <= last_level_index);

            let mut index = first_index & !1; // make the starting index even
            let stop_index = cmp::min(last_index | 1, last_level_index);
            while index < stop_index {
                let key = ProofListKey::new(height, index);
                let branch_hash = HashTag::hash_node(
                    &self.get_branch_unchecked(key),
                    &self.get_branch_unchecked(key.as_right()),
                );
                self.base.put(&key.parent(), branch_hash);
                index += 2;
            }

            if stop_index % 2 == 0 {
                let key = ProofListKey::new(height, stop_index);
                let branch_hash = HashTag::hash_single_node(&self.get_branch_unchecked(key));
                self.base.put(&key.parent(), branch_hash);
            }

            first_index >>= 1;
            last_index >>= 1;
            last_level_index >>= 1;
        }

        debug_assert_eq!(first_index, 0);
        debug_assert_eq!(last_index, 0);
        debug_assert_eq!(last_level_index, 0);
    }

    /// Appends an element to the back of the proof list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    /// ```
    pub fn push(&mut self, value: V) {
        self.extend(iter::once(value));
    }

    /// Extends the proof list with the contents of an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    ///
    /// index.extend([1, 2, 3].iter().cloned());
    /// assert_eq!(3, index.len());
    /// ```
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = V>,
    {
        let first_index = self.len();
        let mut last_index = first_index;

        for value in iter {
            self.base.put(
                &ProofListKey::new(1, last_index),
                HashTag::hash_leaf(&value.to_bytes()),
            );
            self.base.put(&ProofListKey::leaf(last_index), value);
            last_index += 1;
        }

        if last_index == first_index {
            // No elements in the iterator; we're done.
            return;
        }
        self.set_len(last_index);
        self.update_range(first_index, last_index - 1);
    }

    /// Changes a value at the specified position.
    ///
    /// # Panics
    ///
    /// Panics if `index` is equal or greater than the current state of the proof list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.get(0));
    ///
    /// index.set(0, 100);
    /// assert_eq!(Some(100), index.get(0));
    /// ```
    pub fn set(&mut self, index: u64, value: V) {
        if index >= self.len() {
            panic!(
                "Index out of bounds: the len is {} but the index is {}",
                self.len(),
                index
            );
        }
        self.base.put(
            &ProofListKey::new(1, index),
            HashTag::hash_leaf(&value.to_bytes()),
        );
        self.base.put(&ProofListKey::leaf(index), value);
        self.update_range(index, index);
    }

    /// Shortens the list, keeping the indicated number of first `len` elements
    /// and dropping the rest.
    ///
    /// If `len` is greater than the current state of the list, this has no effect.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    /// assert_eq!(5, index.len());
    /// index.truncate(3);
    /// assert!(index.iter().eq(vec![1, 2, 3]));
    /// ```
    pub fn truncate(&mut self, new_length: u64) {
        if self.len() <= new_length {
            return;
        }
        if new_length == 0 {
            self.clear();
            return;
        }

        let mut old_last_index = self.len() - 1;
        let old_height = self.height();
        self.set_len(new_length);
        let mut last_index = Some(new_length - 1);

        // Remove values.
        for index in new_length..=old_last_index {
            self.base.remove(&ProofListKey::leaf(index));
        }

        let mut started_updating_hashes = false;
        for height in 1..old_height {
            // Remove excessive branches on the level.
            for index in last_index.map_or(0, |i| i + 1)..=old_last_index {
                self.base.remove(&ProofListKey::new(height, index));
            }

            // Recalculate the hash of the last element on the next level if it has changed.
            if let Some(last_index) = last_index {
                // We start updating hashes once the `last_index` becomes a single hashed node.
                if last_index > 0 && last_index < old_last_index && last_index % 2 == 0 {
                    started_updating_hashes = true;
                }

                if started_updating_hashes && last_index > 0 {
                    let key = ProofListKey::new(height, last_index);
                    let hash = self.get_branch_unchecked(key);
                    let parent_hash = if key.is_left() {
                        HashTag::hash_single_node(&hash)
                    } else {
                        let left_sibling = self.get_branch_unchecked(key.as_left());
                        HashTag::hash_node(&left_sibling, &hash)
                    };
                    self.base.put(&key.parent(), parent_hash);
                }
            }

            last_index = match last_index {
                Some(0) | None => None,
                Some(i) => Some(i >> 1),
            };
            old_last_index >>= 1;
        }
    }

    /// Removes the last element from the list and returns it, or returns `None`
    /// if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new("list", &fork);
    /// assert_eq!(None, index.pop());
    /// index.push(1);
    /// assert_eq!(Some(1), index.pop());
    /// ```
    pub fn pop(&mut self) -> Option<V> {
        if self.is_empty() {
            None
        } else {
            let last_element = self.get(self.len() - 1); // is always `Some(_)`
            self.truncate(self.len() - 1);
            last_element
        }
    }

    /// Clears the proof list, removing all values.
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
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    /// index.push(1);
    /// assert!(!index.is_empty());
    /// index.clear();
    /// assert!(index.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.base.clear();
        self.state.clear();
    }
}

impl<T, V> ObjectHash for ProofListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    /// Returns a list hash of the proof list or a hash value of the empty list.
    ///
    /// List hash is calculated as follows:
    ///
    /// ```text
    /// h = sha-256( HashTag::List || len as u64 || merkle_root )
    /// ```
    ///
    /// Empty list hash:
    ///
    /// ```text
    /// h = sha-256( HashTag::List || 0 || Hash::default() )
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, ProofListIndex, HashTag, ObjectHash};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &fork);
    ///
    /// let default_hash = index.object_hash();
    /// assert_eq!(HashTag::empty_list_hash(), default_hash);
    /// index.push(1);
    /// let hash = index.object_hash();
    /// assert_ne!(hash, default_hash);
    /// ```
    fn object_hash(&self) -> Hash {
        HashTag::hash_list_node(self.len(), self.merkle_root())
    }
}

impl<'a, T, V> std::iter::IntoIterator for &'a ProofListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    type Item = V;
    type IntoIter = ProofListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for ProofListIndexIter<'a, V>
where
    V: BinaryValue,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(_, v)| v)
    }
}
