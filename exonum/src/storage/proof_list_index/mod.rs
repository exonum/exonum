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

//! An implementation of a Merkelized version of an array list (Merkle tree).

pub use self::proof::{ListProof, ListProofError};

use std::{cell::Cell, marker::PhantomData};

use self::key::ProofListKey;
use super::{
    base_index::{BaseIndex, BaseIndexIter}, indexes_metadata::IndexType, Fork, Snapshot,
    StorageKey, StorageValue,
};
use crypto::{hash, Hash, HashStream};

mod key;
mod proof;
#[cfg(test)]
mod tests;

// TODO: Implement pop and truncate methods for Merkle tree. (ECR-173)

/// A Merkelized version of an array list that provides proofs of existence for the list items.
///
/// `ProofListIndex` implements a Merkle tree, storing elements as leaves and using `u64` as
/// an index. `ProofListIndex` requires that the elements implement the [`StorageValue`] trait.
///
/// [`StorageValue`]: ../trait.StorageValue.html
#[derive(Debug)]
pub struct ProofListIndex<T, V> {
    base: BaseIndex<T>,
    length: Cell<Option<u64>>,
    _v: PhantomData<V>,
}

/// An iterator over the items of a `ProofListIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`ProofListIndex`]. See its documentation for more.
///
/// [`iter`]: struct.ProofListIndex.html#method.iter
/// [`iter_from`]: struct.ProofListIndex.html#method.iter_from
/// [`ProofListIndex`]: struct.ProofListIndex.html
#[derive(Debug)]
pub struct ProofListIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, ProofListKey, V>,
}

fn pair_hash(h1: &Hash, h2: &Hash) -> Hash {
    HashStream::new()
        .update(h1.as_ref())
        .update(h2.as_ref())
        .hash()
}

impl<T, V> ProofListIndex<T, V>
where
    T: AsRef<Snapshot>,
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
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    ///
    /// let snapshot = db.snapshot();
    /// let index: ProofListIndex<_, u8> = ProofListIndex::new(name, &snapshot);
    ///
    /// let mut fork = db.fork();
    /// let mut mut_index: ProofListIndex<_, u8> = ProofListIndex::new(name, &mut fork);
    /// ```
    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {
        ProofListIndex {
            base: BaseIndex::new(index_name, IndexType::ProofList, view),
            length: Cell::new(None),
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
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let index_id = vec![01];
    ///
    /// let snapshot = db.snapshot();
    /// let index: ProofListIndex<_, u8> =
    ///                             ProofListIndex::new_in_family(name, &index_id, &snapshot);
    ///
    /// let mut fork = db.fork();
    /// let mut mut_index : ProofListIndex<_, u8> =
    ///                                 ProofListIndex::new_in_family(name, &index_id, &mut fork);
    /// ```
    pub fn new_in_family<S: AsRef<str>, I: StorageKey>(
        family_name: S,
        index_id: &I,
        view: T,
    ) -> Self {
        ProofListIndex {
            base: BaseIndex::new_in_family(family_name, index_id, IndexType::ProofList, view),
            length: Cell::new(None),
            _v: PhantomData,
        }
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

    /// Returns an element at that position or `None` if out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
    /// assert_eq!(None, index.get(0));
    ///
    /// index.push(10);
    /// assert_eq!(Some(10), index.get(0));
    /// ```
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&ProofListKey::leaf(index))
    }

    /// Returns the last element of the proof list, or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
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
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
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
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
    /// assert_eq!(0, index.len());
    ///
    /// index.push(1);
    /// assert_eq!(1, index.len());
    /// ```
    pub fn len(&self) -> u64 {
        if let Some(len) = self.length.get() {
            return len;
        }
        let len = self.base.get(&()).unwrap_or(0);
        self.length.set(Some(len));
        len
    }

    /// Returns the height of the proof list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
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

    /// Returns the Merkle root hash of the proof list or default hash value if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
    ///
    /// let default_hash = index.merkle_root();
    /// assert_eq!(Hash::default(), default_hash);
    ///
    /// index.push(1);
    /// let hash = index.merkle_root();
    /// assert_ne!(hash, default_hash);
    /// ```
    pub fn merkle_root(&self) -> Hash {
        self.get_branch(self.root_key()).unwrap_or_default()
    }

    /// Returns the proof of existence for the list element at the specified position.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
    ///
    /// index.push(1);
    ///
    /// let proof = index.get_proof(0);
    /// ```
    pub fn get_proof(&self, index: u64) -> ListProof<V> {
        if index >= self.len() {
            panic!(
                "Index out of bounds: the len is {} but the index is {}",
                self.len(),
                index
            );
        }
        self.construct_proof(self.root_key(), index, index + 1)
    }

    /// Returns the proof of existence for the list elements in the specified range.
    ///
    /// # Panics
    ///
    /// Panics if the range is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// let list_proof = index.get_range_proof(1, 3);
    /// ```
    pub fn get_range_proof(&self, from: u64, to: u64) -> ListProof<V> {
        if to > self.len() {
            panic!(
                "Illegal range boundaries: the len is {:?}, but the range end is {:?}",
                self.len(),
                to
            )
        }
        if to <= from {
            panic!(
                "Illegal range boundaries: the range start is {:?}, but the range end is {:?}",
                from, to
            )
        }

        self.construct_proof(self.root_key(), from, to)
    }

    /// Returns an iterator over the list. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
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
            base_iter: self.base.iter(&0u8),
        }
    }

    /// Returns an iterator over the list starting from the specified position. The iterator
    /// element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
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
            base_iter: self.base.iter_from(&0u8, &ProofListKey::leaf(from)),
        }
    }
}

impl<'a, V> ProofListIndex<&'a mut Fork, V>
where
    V: StorageValue,
{
    fn set_len(&mut self, len: u64) {
        self.base.put(&(), len);
        self.length.set(Some(len));
    }

    fn set_branch(&mut self, key: ProofListKey, hash: Hash) {
        debug_assert!(key.height() > 0);

        self.base.put(&key, hash)
    }

    /// Appends an element to the back of the proof list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    /// ```
    pub fn push(&mut self, value: V) {
        let len = self.len();
        self.set_len(len + 1);
        let mut key = ProofListKey::new(1, len);
        self.base.put(&key, value.hash());
        self.base.put(&ProofListKey::leaf(len), value);
        while key.height() < self.height() {
            let hash = if key.is_left() {
                hash(self.get_branch_unchecked(key).as_ref())
            } else {
                pair_hash(
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
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
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
    /// Panics if `index` is equal or greater than the proof list's current length.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
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
        self.base.put(&key, value.hash());
        self.base.put(&ProofListKey::leaf(index), value);
        while key.height() < self.height() {
            let (left, right) = (key.as_left(), key.as_right());
            let hash = if self.has_branch(right) {
                pair_hash(
                    &self.get_branch_unchecked(left),
                    &self.get_branch_unchecked(right),
                )
            } else {
                hash(self.get_branch_unchecked(left).as_ref())
            };
            key = key.parent();
            self.set_branch(key, hash);
        }
    }

    /// Clears the proof list, removing all values.
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
    /// use exonum::storage::{MemoryDB, Database, ProofListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ProofListIndex::new(name, &mut fork);
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    ///
    /// index.clear();
    /// assert!(index.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.length.set(Some(0));
        self.base.clear()
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a ProofListIndex<T, V>
where
    T: AsRef<Snapshot>,
    V: StorageValue,
{
    type Item = V;
    type IntoIter = ProofListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for ProofListIndexIter<'a, V>
where
    V: StorageValue,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(_, v)| v)
    }
}
