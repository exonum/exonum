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

//! An implementation of a set for items that utilize the `BinaryKey` trait.
//!
//! `KeySetIndex` implements a set that stores elements as keys with empty values.
//! The given section contains information on the methods related to `KeySetIndex`
//! and the iterator over the items of this set.

use std::{borrow::Borrow, marker::PhantomData};

use crate::views::IndexAddress;
use crate::{
    views::{AnyObject, IndexAccess, IndexBuilder, IndexState, IndexType, Iter as ViewIter, View},
    BinaryKey, BinaryValue,
};

/// A set of key items.
///
/// `KeySetIndex` implements a set that stores the elements as keys with empty values.
/// `KeySetIndex` requires that elements should implement the [`BinaryKey`] trait.
///
/// [`BinaryKey`]: ../trait.BinaryKey.html
#[derive(Debug)]
pub struct KeySetIndex<T: IndexAccess, K> {
    base: View<T>,
    state: IndexState<T, u64>,
    _k: PhantomData<K>,
}

/// Returns an iterator over the items of a `KeySetIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] method on [`KeySetIndex`]. See its documentation for details.
///
/// [`iter`]: struct.KeySetIndex.html#method.iter
/// [`iter_from`]: struct.KeySetIndex.html#method.iter_from
/// [`KeySetIndex`]: struct.KeySetIndex.html
#[derive(Debug)]
pub struct KeySetIndexIter<'a, K> {
    base_iter: ViewIter<'a, K, ()>,
}

impl<T, K> AnyObject<T> for KeySetIndex<T, K>
where
    T: IndexAccess,
    K: BinaryKey,
{
    fn view(self) -> View<T> {
        self.base
    }

    fn object_type(&self) -> IndexType {
        IndexType::KeySet
    }

    fn metadata(&self) -> Vec<u8> {
        self.state.metadata().to_bytes()
    }
}

impl<T, K> KeySetIndex<T, K>
where
    T: IndexAccess,
    K: BinaryKey,
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
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let snapshot = db.snapshot();
    /// let name = "name";
    /// let index: KeySetIndex<_, u8> = KeySetIndex::new(name, &snapshot);
    /// ```
    pub fn new<S: Into<String>>(index_name: S, view: T) -> Self {
        let (base, state) = IndexBuilder::new(view)
            .index_type(IndexType::KeySet)
            .index_name(index_name)
            .build();

        Self {
            base,
            state,
            _k: PhantomData,
        }
    }

    /// Creates a new index representation based on the name, index ID in family
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
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let snapshot = db.snapshot();
    /// let name = "name";
    /// let index_id = vec![123];
    /// let index: KeySetIndex<_, u8> = KeySetIndex::new_in_family(name, &index_id, &snapshot);
    /// ```
    pub fn new_in_family<S, I>(family_name: S, index_id: &I, view: T) -> Self
    where
        I: BinaryKey,
        I: ?Sized,
        S: Into<String>,
    {
        let (base, state) = IndexBuilder::new(view)
            .index_type(IndexType::KeySet)
            .index_name(family_name)
            .family_id(index_id)
            .build();

        Self {
            base,
            state,
            _k: PhantomData,
        }
    }

    pub fn get_from<I: Into<IndexAddress>>(address: I, access: T) -> Option<Self> {
        IndexBuilder::from_address(address, access)
            .index_type(IndexType::KeySet)
            .build_existed()
            .map(|(base, state)| Self {
                base,
                state,
                _k: PhantomData,
            })
    }

    pub fn create_from<I: Into<IndexAddress>>(address: I, access: T) -> Self {
        let (base, state) = IndexBuilder::from_address(address, access)
            .index_type(IndexType::KeySet)
            .build();

        Self {
            base,
            state,
            _k: PhantomData,
        }
    }

    /// Returns `true` if the set contains the indicated value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &fork);
    /// assert!(!index.contains(&1));
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn contains<Q>(&self, item: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: BinaryKey + ?Sized,
    {
        self.base.contains(item)
    }

    /// Returns an iterator visiting all elements in ascending order. The iterator element type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
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

    /// Returns an iterator visiting all elements in arbitrary order starting from the specified value.
    /// The iterator element type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
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

    /// Adds a key to the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]
    pub fn insert(&mut self, item: K) {
        self.base.put(&item, ());
        self.set_len(self.len() + 1);
    }

    /// Removes a key from the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &fork);
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
        Q: BinaryKey + ?Sized,
    {
        self.base.remove(item);
        self.set_len(self.len().saturating_sub(1));
    }

    /// Clears the set, removing all values.
    ///
    /// # Notes
    /// Currently, this method is not optimized to delete a large set of data. During the execution of
    /// this method, the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    ///
    /// index.clear();
    /// assert!(!index.contains(&1));
    /// ```
    pub fn clear(&mut self) {
        self.base.clear();
        self.state.clear();
    }

    /// Returns the number of elements in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = KeySetIndex::new("index", &fork);
    /// assert_eq!(0, index.len());
    ///
    /// index.put(&1, 10);
    ///
    /// assert_eq!(1, index.len());
    /// ```
    pub fn len(&self) -> u64 {
        self.state.get()
    }

    /// Returns `true` if the map contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &fork);
    /// assert!(index.is_empty());
    ///
    /// index.put(&0, 10);
    /// assert!(!index.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn set_len(&mut self, len: u64) {
        self.state.set(len)
    }
}

impl<'a, T, K> ::std::iter::IntoIterator for &'a KeySetIndex<T, K>
where
    T: IndexAccess,
    K: BinaryKey,
{
    type Item = K::Owned;
    type IntoIter = KeySetIndexIter<'a, K>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K> Iterator for KeySetIndexIter<'a, K>
where
    K: BinaryKey,
{
    type Item = K::Owned;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, TemporaryDB};

    const INDEX_NAME: &str = "test_index_name";

    #[test]
    fn str_key() {
        const KEY: &str = "key_1";
        let db = TemporaryDB::new();
        let fork = db.fork();

        let mut index: KeySetIndex<_, String> = KeySetIndex::new(INDEX_NAME, &fork);

        assert_eq!(false, index.contains(KEY));

        index.insert(KEY.to_owned());
        assert_eq!(true, index.contains(KEY));

        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    #[test]
    fn u8_slice_key() {
        const KEY: &[u8] = &[1, 2, 3];
        let db = TemporaryDB::new();
        let fork = db.fork();

        let mut index: KeySetIndex<_, Vec<u8>> = KeySetIndex::new(INDEX_NAME, &fork);
        assert_eq!(false, index.contains(KEY));

        index.insert(KEY.to_owned());
        assert_eq!(true, index.contains(KEY));

        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    #[test]
    fn key_set_methods() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut index = KeySetIndex::new(INDEX_NAME, &fork);

        assert!(!index.contains(&1_u8));
        assert_eq!(index.len(), 0);

        index.insert(1_u8);
        assert_eq!(index.len(), 1);
        assert!(index.contains(&1_u8));

        index.insert(2_u8);
        assert_eq!(index.len(), 2);

        let key = index.iter().next().unwrap();
        index.remove(&key);

        assert_eq!(index.len(), 1);
        assert!(!index.contains(&1_u8));

        index.clear();
        assert!(index.is_empty());
    }
}
