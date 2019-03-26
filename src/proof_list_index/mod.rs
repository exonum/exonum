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

pub use self::proof::{ListProof, ListProofError};

use std::{
    marker::PhantomData,
    ops::{Bound, RangeBounds},
};

use exonum_crypto::Hash;

use self::{key::ProofListKey, proof::ProofOfAbsence};
use crate::views::IndexAddress;
use crate::{
    hash::HashTag,
    views::{AnyObject, IndexAccess, IndexBuilder, IndexState, IndexType, Iter as ViewIter, View},
    BinaryKey, BinaryValue, ObjectHash,
};

mod key;
mod proof;
#[cfg(test)]
mod tests;

// TODO: Implement pop and truncate methods for Merkle tree. (ECR-173)

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
    ///                             ProofListIndex::new_in_family(name, &index_id, &snapshot);
    ///
    /// let fork = db.fork();
    /// let mut mut_index : ProofListIndex<_, u8> =
    ///                                 ProofListIndex::new_in_family(name, &index_id, &fork);
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

    pub fn get_from_view(view: View<T>) -> Option<Self> {
        IndexBuilder::for_view(view)
            .index_type(IndexType::ProofList)
            .build_existed()
            .map(|(base, state)| Self {
                base,
                state,
                _v: PhantomData,
            })
    }

    pub fn create_from_view(view: View<T>) -> Self {
        let (base, state) = IndexBuilder::for_view(view)
            .index_type(IndexType::ProofList)
            .build();

        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    pub fn create_from<I: Into<IndexAddress>>(address: I, access: T) -> Self {
        let (base, state) = IndexBuilder::from_address(address, access)
            .index_type(IndexType::ProofList)
            .build();

        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    pub fn get_from<I: Into<IndexAddress>>(address: I, access: T) -> Option<Self> {
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
        debug_assert!(key.height() > 0);

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

    fn construct_proof(&self, key: ProofListKey, from: u64, to: u64) -> ListProof<V> {
        if key.height() == 1 {
            return ListProof::Leaf(self.get(key.index()).unwrap());
        }
        let middle = key.first_right_leaf_index();
        if to <= middle {
            ListProof::Left(
                Box::new(self.construct_proof(key.left(), from, to)),
                self.get_branch(key.right()),
            )
        } else if middle <= from {
            ListProof::Right(
                self.get_branch_unchecked(key.left()),
                Box::new(self.construct_proof(key.right(), from, to)),
            )
        } else {
            ListProof::Full(
                Box::new(self.construct_proof(key.left(), from, middle)),
                Box::new(self.construct_proof(key.right(), middle, to)),
            )
        }
    }

    fn merkle_root(&self) -> Hash {
        self.get_branch(self.root_key()).unwrap_or_default()
    }

    fn set_len(&mut self, len: u64) {
        self.state.set(len)
    }

    fn set_branch(&mut self, key: ProofListKey, hash: Hash) {
        debug_assert!(key.height() > 0);

        self.base.put(&key, hash)
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

    /// Returns the height of the proof list.
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
    /// assert_eq!(1, index.height());
    ///
    /// index.push(1);
    /// assert_eq!(1, index.len());
    ///
    /// index.push(1);
    /// assert_eq!(2, index.len());
    /// ```
    pub fn height(&self) -> u8 {
        self.len().next_power_of_two().trailing_zeros() as u8 + 1
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
    ///
    /// let proof = index.get_proof(0);
    ///
    /// let proof_of_absence = index.get_proof(1);
    /// ```
    pub fn get_proof(&self, index: u64) -> ListProof<V> {
        if index >= self.len() {
            return ListProof::Absent(ProofOfAbsence::new(self.len(), self.merkle_root()));
        }

        self.construct_proof(self.root_key(), index, index + 1)
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
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// let list_proof = index.get_range_proof(1..3);
    ///
    /// // Range (1..10) doesn't exist in index.
    /// let list_proof_of_absence = index.get_range_proof(1..10);
    ///
    /// ```
    pub fn get_range_proof<R: RangeBounds<u64>>(&self, range: R) -> ListProof<V> {
        let from = match range.start_bound() {
            Bound::Unbounded => 0_u64,
            Bound::Included(from) | Bound::Excluded(from) => *from,
        };

        let to = match range.end_bound() {
            Bound::Unbounded => self.len(),
            Bound::Included(to) | Bound::Excluded(to) => *to,
        };

        if to <= from {
            panic!(
                "Illegal range boundaries: the range start is {:?}, but the range end is {:?}",
                from, to
            )
        }

        if to > self.len() {
            ListProof::Absent(ProofOfAbsence::new(self.len(), self.merkle_root()))
        } else {
            self.construct_proof(self.root_key(), from, to)
        }
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
        let len = self.len();
        self.set_len(len + 1);
        let mut key = ProofListKey::new(1, len);

        self.base.put(&key, HashTag::hash_leaf(&value.to_bytes()));
        self.base.put(&ProofListKey::leaf(len), value);
        while key.height() < self.height() {
            let hash = if key.is_left() {
                HashTag::hash_single_node(&self.get_branch_unchecked(key))
            } else {
                HashTag::hash_node(
                    &self.get_branch_unchecked(key.as_left()),
                    &self.get_branch_unchecked(key),
                )
            };
            key = key.parent();
            self.set_branch(key, hash);
        }
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
        for value in iter {
            self.push(value)
        }
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
        let mut key = ProofListKey::new(1, index);
        self.base.put(&key, HashTag::hash_leaf(&value.to_bytes()));
        self.base.put(&ProofListKey::leaf(index), value);
        while key.height() < self.height() {
            let (left, right) = (key.as_left(), key.as_right());
            let hash = if self.has_branch(right) {
                HashTag::hash_node(
                    &self.get_branch_unchecked(left),
                    &self.get_branch_unchecked(right),
                )
            } else {
                HashTag::hash_single_node(&self.get_branch_unchecked(left))
            };
            key = key.parent();
            self.set_branch(key, hash);
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
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    ///
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
    V: BinaryValue + ObjectHash,
{
    /// Returns a list hash of the proof list or a hash value of the empty list.
    ///
    /// List hash is calculated as follows:
    /// ```text
    /// h = sha-256( HashTag::List || len as u64 || merkle_root )
    /// ```
    /// Empty list hash:
    /// ```text
    /// h = sha-256( HashTag::List || 0 || Hash::default() )
    /// ```
    ///
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
    ///
    /// index.push(1);
    /// let hash = index.object_hash();
    /// assert_ne!(hash, default_hash);
    /// ```
    fn object_hash(&self) -> Hash {
        HashTag::hash_list_node(self.len(), self.merkle_root())
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a ProofListIndex<T, V>
where
    T: IndexAccess,
    V: BinaryValue + ObjectHash,
{
    type Item = V;
    type IntoIter = ProofListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for ProofListIndexIter<'a, V>
where
    V: BinaryValue + ObjectHash,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(_, v)| v)
    }
}
