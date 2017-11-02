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

//! An implementation of set for items that implement `StorageKey` trait.
use std::marker::PhantomData;

use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageKey};

/// A set of items that implement `StorageKey` trait.
///
/// `KeySetIndex` implements a set, storing the elements as keys with empty values.
/// `KeySetIndex` requires that the elements implement the [`StorageKey`] trait.
/// [`StorageKey`]: ../trait.StorageKey.html
#[derive(Debug)]
pub struct KeySetIndex<T, K> {
    base: BaseIndex<T>,
    _k: PhantomData<K>,
}

/// An iterator over the items of a `KeySetIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`KeySetIndex`]. See its documentation for more.
///
/// [`iter`]: struct.KeySetIndex.html#method.iter
/// [`iter_from`]: struct.KeySetIndex.html#method.iter_from
/// [`KeySetIndex`]: struct.KeySetIndex.html
#[derive(Debug)]
pub struct KeySetIndexIter<'a, K> {
    base_iter: BaseIndexIter<'a, K, ()>,
}

impl<T, K> KeySetIndex<T, K> {
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
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let name = "name";
    /// let index: KeySetIndex<_, u8> = KeySetIndex::new(name, &snapshot);
    /// # drop(index);
    /// ```
    pub fn new<S: AsRef<str>>(name: S, view: T) -> Self {
        KeySetIndex {
            base: BaseIndex::new(name, view),
            _k: PhantomData,
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
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let name = "name";
    /// let prefix = vec![123];
    /// let index: KeySetIndex<_, u8> = KeySetIndex::with_prefix(name, prefix, &snapshot);
    /// # drop(index);
    /// ```
    pub fn with_prefix<S: AsRef<str>>(name: S, prefix: Vec<u8>, view: T) -> Self {
        KeySetIndex {
            base: BaseIndex::with_prefix(name, prefix, view),
            _k: PhantomData,
        }
    }
}

impl<T, K> KeySetIndex<T, K>
where
    T: AsRef<Snapshot>,
    K: StorageKey,
{
    /// Returns `true` if the set contains a value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &mut fork);
    /// assert!(!index.contains(&1));
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn contains(&self, item: &K) -> bool {
        self.base.contains(item)
    }

    /// An iterator visiting all elements in ascending order. The iterator element type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: KeySetIndex<_, u8> = KeySetIndex::new(name, &snapshot);
    ///
    /// for val in index.iter() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter(&self) -> KeySetIndexIter<K> {
        KeySetIndexIter { base_iter: self.base.iter(&()) }
    }

    /// An iterator visiting all elements in arbitrary order starting from the specified value.
    /// The iterator element type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: KeySetIndex<_, u8> = KeySetIndex::new(name, &snapshot);
    ///
    /// for val in index.iter_from(&2) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: &K) -> KeySetIndexIter<K> {
        KeySetIndexIter { base_iter: self.base.iter_from(&(), from) }
    }
}

impl<'a, K> KeySetIndex<&'a mut Fork, K>
where
    K: StorageKey,
{
    /// Adds a value to the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &mut fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn insert(&mut self, item: K) {
        self.base.put(&item, ())
    }

    /// Removes a value from the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &mut fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    ///
    /// index.remove(&1);
    /// assert!(!index.contains(&1));
    /// ```
    pub fn remove(&mut self, item: &K) {
        self.base.remove(item)
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
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &mut fork);
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

impl<'a, T, K> ::std::iter::IntoIterator for &'a KeySetIndex<T, K>
where
    T: AsRef<Snapshot>,
    K: StorageKey,
{
    type Item = K;
    type IntoIter = KeySetIndexIter<'a, K>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K> Iterator for KeySetIndexIter<'a, K>
where
    K: StorageKey,
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}
