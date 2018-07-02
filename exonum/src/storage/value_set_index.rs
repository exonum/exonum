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

//! An implementation of set for items that implement `StorageValue` trait.

use std::marker::PhantomData;

use super::{
    base_index::{BaseIndex, BaseIndexIter}, indexes_metadata::IndexType, Fork, Snapshot,
    StorageKey, StorageValue,
};
use crypto::Hash;

/// A set of items that implement `StorageValue` trait.
///
/// `ValueSetIndex` implements a set, storing the element as values using its hash as a key.
/// `ValueSetIndex` requires that the elements implement the [`StorageValue`] trait.
///
/// [`StorageValue`]: ../trait.StorageValue.html
#[derive(Debug)]
pub struct ValueSetIndex<T, V> {
    base: BaseIndex<T>,
    _v: PhantomData<V>,
}

/// An iterator over the items of a `ValueSetIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`ValueSetIndex`]. See its documentation for more.
///
/// [`iter`]: struct.ValueSetIndex.html#method.iter
/// [`iter_from`]: struct.ValueSetIndex.html#method.iter_from
/// [`ValueSetIndex`]: struct.ValueSetIndex.html
#[derive(Debug)]
pub struct ValueSetIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, Hash, V>,
}

/// An iterator over the hashes of items of a `ValueSetIndex`.
///
/// This struct is created by the [`hashes`] or
/// [`hashes_from`] methods on [`ValueSetIndex`]. See its documentation for more.
///
/// [`hashes`]: struct.ValueSetIndex.html#method.iter
/// [`hashes_from`]: struct.ValueSetIndex.html#method.iter_from
/// [`ValueSetIndex`]: struct.ValueSetIndex.html
#[derive(Debug)]
pub struct ValueSetIndexHashes<'a> {
    base_iter: BaseIndexIter<'a, Hash, ()>,
}

impl<T, V> ValueSetIndex<T, V>
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
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let snapshot = db.snapshot();
    /// let index: ValueSetIndex<_, u8> = ValueSetIndex::new(name, &snapshot);
    /// ```
    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {
        ValueSetIndex {
            base: BaseIndex::new(index_name, IndexType::ValueSet, view),
            _v: PhantomData,
        }
    }

    /// Creates a new index representation based on the name, index id in family
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
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let name = "name";
    /// let index_id = vec![123];
    /// let index: ValueSetIndex<_, u8> = ValueSetIndex::new_in_family(name, &index_id, &snapshot);
    /// ```
    pub fn new_in_family<S: AsRef<str>, I: StorageKey>(
        family_name: S,
        index_id: &I,
        view: T,
    ) -> Self {
        ValueSetIndex {
            base: BaseIndex::new_in_family(family_name, index_id, IndexType::ValueSet, view),
            _v: PhantomData,
        }
    }

    /// Returns `true` if the set contains a value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let mut fork = db.fork();
    /// let mut index = ValueSetIndex::new(name, &mut fork);
    /// assert!(!index.contains(&1));
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn contains(&self, item: &V) -> bool {
        self.contains_by_hash(&item.hash())
    }

    /// Returns `true` if the set contains a value with the specified hash.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    /// use exonum::crypto;
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let mut fork = db.fork();
    /// let mut index = ValueSetIndex::new(name, &mut fork);
    ///
    /// let data = vec![1, 2, 3];
    /// let data_hash = crypto::hash(&data);
    /// assert!(!index.contains_by_hash(&data_hash));
    ///
    /// index.insert(data);
    /// assert!(index.contains_by_hash(&data_hash));
    pub fn contains_by_hash(&self, hash: &Hash) -> bool {
        self.base.contains(hash)
    }

    /// An iterator visiting all elements in arbitrary order. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let snapshot = db.snapshot();
    /// let index: ValueSetIndex<_, u8> = ValueSetIndex::new(name, &snapshot);
    ///
    /// for val in index.iter() {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter(&self) -> ValueSetIndexIter<V> {
        ValueSetIndexIter {
            base_iter: self.base.iter(&()),
        }
    }

    /// An iterator visiting all elements in arbitrary order starting from the specified hash of
    /// a value. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let snapshot = db.snapshot();
    /// let index: ValueSetIndex<_, u8> = ValueSetIndex::new(name, &snapshot);
    ///
    /// let hash = Hash::default();
    ///
    /// for val in index.iter_from(&hash) {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: &Hash) -> ValueSetIndexIter<V> {
        ValueSetIndexIter {
            base_iter: self.base.iter_from(&(), from),
        }
    }

    /// An iterator visiting hashes of all elements in ascending order. The iterator element type
    /// is [Hash](../../crypto/struct.Hash.html).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let snapshot = db.snapshot();
    /// let index: ValueSetIndex<_, u8> = ValueSetIndex::new(name, &snapshot);
    ///
    /// for val in index.hashes() {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn hashes(&self) -> ValueSetIndexHashes {
        ValueSetIndexHashes {
            base_iter: self.base.iter(&()),
        }
    }

    /// An iterator visiting hashes of all elements in ascending order starting from the specified
    /// hash. The iterator element type is [Hash](../../crypto/struct.Hash.html).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    /// use exonum::crypto::Hash;
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let snapshot = db.snapshot();
    /// let index: ValueSetIndex<_, u8> = ValueSetIndex::new(name, &snapshot);
    ///
    /// let hash = Hash::default();
    ///
    /// for val in index.hashes_from(&hash) {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn hashes_from(&self, from: &Hash) -> ValueSetIndexHashes {
        ValueSetIndexHashes {
            base_iter: self.base.iter_from(&(), from),
        }
    }
}

impl<'a, V> ValueSetIndex<&'a mut Fork, V>
where
    V: StorageValue,
{
    /// Adds a value to the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let mut fork = db.fork();
    /// let mut index = ValueSetIndex::new(name, &mut fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn insert(&mut self, item: V) {
        self.base.put(&item.hash(), item)
    }

    /// Removes a value from the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let mut fork = db.fork();
    /// let mut index = ValueSetIndex::new(name, &mut fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    ///
    /// index.remove(&1);
    /// assert!(!index.contains(&1));
    /// ```
    pub fn remove(&mut self, item: &V) {
        self.remove_by_hash(&item.hash())
    }

    /// Removes a value from the set by the specified hash.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    /// use exonum::crypto;
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let mut fork = db.fork();
    /// let mut index = ValueSetIndex::new(name, &mut fork);
    ///
    /// let data = vec![1, 2, 3];
    /// let data_hash = crypto::hash(&data);
    /// index.insert(data);
    /// assert!(index.contains_by_hash(&data_hash));
    ///
    /// index.remove_by_hash(&data_hash);
    /// assert!(!index.contains_by_hash(&data_hash));
    pub fn remove_by_hash(&mut self, hash: &Hash) {
        self.base.remove(hash)
    }

    /// Clears the set, removing all values.
    ///
    /// # Notes
    /// Currently this method is not optimized to delete large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ValueSetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name  = "name";
    /// let mut fork = db.fork();
    /// let mut index = ValueSetIndex::new(name, &mut fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    ///
    /// index.clear();
    /// assert!(!index.contains(&1));
    /// ```
    pub fn clear(&mut self) {
        self.base.clear()
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a ValueSetIndex<T, V>
where
    T: AsRef<Snapshot>,
    V: StorageValue,
{
    type Item = (Hash, V);
    type IntoIter = ValueSetIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for ValueSetIndexIter<'a, V>
where
    V: StorageValue,
{
    type Item = (Hash, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

impl<'a> Iterator for ValueSetIndexHashes<'a> {
    type Item = Hash;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}
