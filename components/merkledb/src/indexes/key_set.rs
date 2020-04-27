// Copyright 2020 The Exonum Team
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

use std::marker::PhantomData;

use crate::{
    access::{Access, AccessError, FromAccess},
    indexes::iter::{Entries, IndexIterator, Keys},
    views::{IndexAddress, IndexType, RawAccess, RawAccessMut, View, ViewWithMetadata},
    BinaryKey,
};

/// A set of key items.
///
/// `KeySetIndex` implements a set that stores the elements as keys with empty values.
/// `KeySetIndex` requires that elements should implement the [`BinaryKey`] trait.
///
/// [`BinaryKey`]: ../trait.BinaryKey.html
#[derive(Debug)]
pub struct KeySetIndex<T: RawAccess, K: ?Sized> {
    base: View<T>,
    _k: PhantomData<K>,
}

impl<T, K> FromAccess<T> for KeySetIndex<T::Base, K>
where
    T: Access,
    K: BinaryKey + ?Sized,
{
    fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError> {
        let view = access.get_or_create_view(addr, IndexType::KeySet)?;
        Ok(Self::new(view))
    }
}

impl<T, K> KeySetIndex<T, K>
where
    T: RawAccess,
    K: BinaryKey + ?Sized,
{
    fn new(view: ViewWithMetadata<T>) -> Self {
        let base = view.into();
        Self {
            base,
            _k: PhantomData,
        }
    }

    /// Returns `true` if the set contains the indicated value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_key_set("name");
    /// assert!(!index.contains(&1));
    ///
    /// index.insert(&1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn contains(&self, item: &K) -> bool {
        self.base.contains(item)
    }

    /// Returns an iterator over set elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let index = fork.get_key_set::<_, u8>("name");
    ///
    /// for val in index.iter() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter(&self) -> Keys<'_, K> {
        self.index_iter(None).skip_values()
    }

    /// Returns an iterator over set elements starting from the specified value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let index = fork.get_key_set::<_, u8>("name");
    ///
    /// for val in index.iter_from(&2) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: &K) -> Keys<'_, K> {
        self.index_iter(Some(from)).skip_values()
    }
}

impl<T, K> KeySetIndex<T, K>
where
    T: RawAccessMut,
    K: BinaryKey + ?Sized,
{
    /// Adds a key to the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_key_set("name");
    ///
    /// index.insert(&1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn insert(&mut self, item: &K) {
        self.base.put(item, ())
    }

    /// Removes a key from the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_key_set("name");
    ///
    /// index.insert(&1);
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
    /// Currently, this method is not optimized to delete a large set of data. During the execution of
    /// this method, the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, KeySetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_key_set("name");
    ///
    /// index.insert(&1);
    /// assert!(index.contains(&1));
    ///
    /// index.clear();
    /// assert!(!index.contains(&1));
    /// ```
    pub fn clear(&mut self) {
        self.base.clear()
    }
}

impl<'a, T, K> IntoIterator for &'a KeySetIndex<T, K>
where
    T: RawAccess,
    K: BinaryKey + ?Sized,
{
    type Item = K::Owned;
    type IntoIter = Keys<'a, K>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T, K> IndexIterator for KeySetIndex<T, K>
where
    T: RawAccess,
    K: BinaryKey + ?Sized,
{
    type Key = K;
    type Value = ();

    fn index_iter(&self, from: Option<&K>) -> Entries<'_, K, ()> {
        Entries::new(&self.base, from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{access::CopyAccessExt, Database, TemporaryDB};

    const INDEX_NAME: &str = "test_index_name";

    #[test]
    fn str_key() {
        const KEY: &str = "key_1";
        let db = TemporaryDB::new();
        let fork = db.fork();

        let mut index: KeySetIndex<_, str> = fork.get_key_set(INDEX_NAME);
        assert_eq!(false, index.contains(KEY));
        index.insert(KEY);
        assert_eq!(true, index.contains(KEY));
        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    #[test]
    fn u8_slice_key() {
        const KEY: &[u8] = &[1, 2, 3];
        let db = TemporaryDB::new();
        let fork = db.fork();

        let mut index = fork.get_key_set(INDEX_NAME);
        assert_eq!(false, index.contains(KEY));
        index.insert(KEY);
        assert_eq!(true, index.contains(KEY));
        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    #[test]
    fn key_set_methods() {
        let db = TemporaryDB::default();
        let fork = db.fork();

        let mut index = fork.get_key_set(INDEX_NAME);
        assert!(!index.contains(&1_u8));
        index.insert(&1_u8);
        assert!(index.contains(&1_u8));
        index.insert(&2_u8);
        let key = index.iter().next().unwrap();
        index.remove(&key);
        assert!(!index.contains(&1_u8));
        index.clear();
        assert!(!index.contains(&2_u8));
    }

    #[test]
    fn no_infinite_iteration_in_flushed_fork() {
        let db = TemporaryDB::new();
        let mut fork = db.fork();
        {
            let mut set = fork.get_key_set::<_, u8>(INDEX_NAME);
            set.insert(&4);
            set.clear();
        }
        fork.flush();
        {
            let mut set = fork.get_key_set::<_, u8>(INDEX_NAME);
            set.remove(&1);
        }
        fork.flush();

        let set = fork.get_key_set::<_, u8>(INDEX_NAME);
        let items: Vec<_> = set.iter().collect();
        assert!(items.is_empty());
    }
}
