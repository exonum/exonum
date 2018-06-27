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

//! An implementation of set for items that implement `StorageKey` trait.

use std::{borrow::Borrow, marker::PhantomData};

use super::{
    base_index::{BaseIndex, BaseIndexIter}, indexes_metadata::IndexType, Fork, Snapshot, StorageKey,
};

/// A set of items that implement `StorageKey` trait.
///
/// `KeySetIndex` implements a set, storing the elements as keys with empty values.
/// `KeySetIndex` requires that the elements implement the [`StorageKey`] trait.
///
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

impl<T, K> KeySetIndex<T, K>
where
    T: AsRef<Snapshot>,
    K: StorageKey,
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
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let name = "name";
    /// let index: KeySetIndex<_, u8> = KeySetIndex::new(name, &snapshot);
    /// ```
    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {
        KeySetIndex {
            base: BaseIndex::new(index_name, IndexType::KeySet, view),
            _k: PhantomData,
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
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let name = "name";
    /// let index_id = vec![123];
    /// let index: KeySetIndex<_, u8> = KeySetIndex::new_in_family(name, &index_id, &snapshot);
    /// ```
    pub fn new_in_family<S: AsRef<str>, I: StorageKey>(
        family_name: S,
        index_id: &I,
        view: T,
    ) -> Self {
        KeySetIndex {
            base: BaseIndex::new_in_family(family_name, index_id, IndexType::KeySet, view),
            _k: PhantomData,
        }
    }

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
    pub fn contains<Q>(&self, item: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: StorageKey + ?Sized,
    {
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
        KeySetIndexIter {
            base_iter: self.base.iter(&()),
        }
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
        KeySetIndexIter {
            base_iter: self.base.iter_from(&(), from),
        }
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
    #[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
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
    pub fn remove<Q>(&mut self, item: &Q)
    where
        K: Borrow<Q>,
        Q: StorageKey + ?Sized,
    {
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
    type Item = K::Owned;
    type IntoIter = KeySetIndexIter<'a, K>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K> Iterator for KeySetIndexIter<'a, K>
where
    K: StorageKey,
{
    type Item = K::Owned;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Database, MemoryDB};
    use super::*;

    const INDEX_NAME: &str = "test_index_name";

    #[test]
    fn str_key() {
        let db = MemoryDB::new();
        let mut fork = db.fork();

        const KEY: &str = "key_1";

        let mut index: KeySetIndex<_, String> = KeySetIndex::new(INDEX_NAME, &mut fork);
        assert_eq!(false, index.contains(KEY));

        index.insert(KEY.to_owned());
        assert_eq!(true, index.contains(KEY));

        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    #[test]
    fn u8_slice_key() {
        let db = MemoryDB::new();
        let mut fork = db.fork();

        const KEY: &[u8] = &[1, 2, 3];

        let mut index: KeySetIndex<_, Vec<u8>> = KeySetIndex::new(INDEX_NAME, &mut fork);
        assert_eq!(false, index.contains(KEY));

        index.insert(KEY.to_owned());
        assert_eq!(true, index.contains(KEY));

        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }
}
