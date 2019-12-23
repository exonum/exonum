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

//! An implementation of a set of items that utilize the `BinaryValue` trait.
//!
//! `ValueSetIndex` implements a set, storing an element as a value and using
//! its hash as a key. The given section contains methods related to `ValueSetIndex`
//! and iterators over the items of this set.

use std::marker::PhantomData;

use exonum_crypto::Hash;

use crate::{
    access::{Access, AccessError, FromAccess},
    views::{
        IndexAddress, IndexType, Iter as ViewIter, RawAccess, RawAccessMut, View, ViewWithMetadata,
    },
    BinaryValue, ObjectHash,
};

/// A set of value items.
///
/// `ValueSetIndex` implements a set, storing an element as a value and using its hash as a key.
/// `ValueSetIndex` requires that elements should implement the [`BinaryValue`] trait.
///
/// [`BinaryValue`]: ../../trait.BinaryValue.html
#[derive(Debug)]
pub struct ValueSetIndex<T: RawAccess, V> {
    base: View<T>,
    _v: PhantomData<V>,
}

/// Returns an iterator over the items of a `ValueSetIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] method on [`ValueSetIndex`]. See its documentation for details.
///
/// [`iter`]: struct.ValueSetIndex.html#method.iter
/// [`iter_from`]: struct.ValueSetIndex.html#method.iter_from
/// [`ValueSetIndex`]: struct.ValueSetIndex.html
#[derive(Debug)]
pub struct Iter<'a, V> {
    base_iter: ViewIter<'a, Hash, V>,
}

/// Returns an iterator over the hashes of items of a `ValueSetIndex`.
///
/// This struct is created by the [`hashes`] or
/// [`hashes_from`] method on [`ValueSetIndex`]. See its documentation for details.
///
/// [`hashes`]: struct.ValueSetIndex.html#method.iter
/// [`hashes_from`]: struct.ValueSetIndex.html#method.iter_from
/// [`ValueSetIndex`]: struct.ValueSetIndex.html
#[derive(Debug)]
pub struct Hashes<'a> {
    base_iter: ViewIter<'a, Hash, ()>,
}

impl<T, V> FromAccess<T> for ValueSetIndex<T::Base, V>
where
    T: Access,
    V: BinaryValue + ObjectHash,
{
    fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError> {
        let view = access.get_or_create_view(addr, IndexType::ValueSet)?;
        Ok(Self::new(view))
    }
}

impl<T, V> ValueSetIndex<T, V>
where
    T: RawAccess,
    V: BinaryValue + ObjectHash,
{
    fn new(view: ViewWithMetadata<T>) -> Self {
        let base = view.into();
        Self {
            base,
            _v: PhantomData,
        }
    }

    /// Returns `true` if the set contains the indicated value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_value_set("name");
    /// assert!(!index.contains(&1));
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn contains(&self, item: &V) -> bool {
        self.contains_by_hash(&item.object_hash())
    }

    /// Returns `true` if the set contains a value with the specified hash.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    /// use exonum_crypto;
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_value_set("name");
    ///
    /// let data = vec![1, 2, 3];
    /// let data_hash = exonum_crypto::hash(&data);
    /// assert!(!index.contains_by_hash(&data_hash));
    ///
    /// index.insert(data);
    /// assert!(index.contains_by_hash(&data_hash));
    /// ```
    pub fn contains_by_hash(&self, hash: &Hash) -> bool {
        self.base.contains(hash)
    }

    /// Returns an iterator visiting all elements in arbitrary order. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let index: ValueSetIndex<_, u8> = fork.get_value_set("name");
    ///
    /// for val in index.iter() {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter(&self) -> Iter<'_, V> {
        Iter {
            base_iter: self.base.iter(&()),
        }
    }

    /// Returns an iterator visiting all elements in arbitrary order starting from the specified hash of
    /// a value. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let index: ValueSetIndex<_, u8> = fork.get_value_set("name");
    ///
    /// let hash = Hash::default();
    ///
    /// for val in index.iter_from(&hash) {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: &Hash) -> Iter<'_, V> {
        Iter {
            base_iter: self.base.iter_from(&(), from),
        }
    }

    /// Returns an iterator visiting hashes of all elements in ascending order. The iterator element type
    /// is [Hash](../../../exonum_crypto/struct.Hash.html).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let index: ValueSetIndex<_, u8> = fork.get_value_set("name");
    ///
    /// for val in index.hashes() {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn hashes(&self) -> Hashes<'_> {
        Hashes {
            base_iter: self.base.iter(&()),
        }
    }

    /// Returns an iterator visiting hashes of all elements in ascending order starting from the specified
    /// hash. The iterator element type is [Hash](../../../exonum_crypto/struct.Hash.html).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    /// use exonum_crypto::Hash;
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let index: ValueSetIndex<_, u8> = fork.get_value_set("name");
    ///
    /// let hash = Hash::default();
    ///
    /// for val in index.hashes_from(&hash) {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn hashes_from(&self, from: &Hash) -> Hashes<'_> {
        Hashes {
            base_iter: self.base.iter_from(&(), from),
        }
    }
}

impl<T, V> ValueSetIndex<T, V>
where
    T: RawAccessMut,
    V: BinaryValue + ObjectHash,
{
    /// Adds a value to the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_value_set("name");
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn insert(&mut self, item: V) {
        self.base.put(&item.object_hash(), item)
    }

    /// Removes a value from the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_value_set("name");
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    ///
    /// index.remove(&1);
    /// assert!(!index.contains(&1));
    /// ```
    pub fn remove(&mut self, item: &V) {
        self.remove_by_hash(&item.object_hash())
    }

    /// Removes a value corresponding to the specified hash from the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    /// use exonum_crypto;
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_value_set("name");
    ///
    /// let data = vec![1, 2, 3];
    /// let data_hash = exonum_crypto::hash(&data);
    /// index.insert(data);
    /// assert!(index.contains_by_hash(&data_hash));
    ///
    /// index.remove_by_hash(&data_hash);
    /// assert!(!index.contains_by_hash(&data_hash));
    /// ```
    pub fn remove_by_hash(&mut self, hash: &Hash) {
        self.base.remove(hash)
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
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, ValueSetIndex};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_value_set("name");
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

impl<'a, T, V> std::iter::IntoIterator for &'a ValueSetIndex<T, V>
where
    T: RawAccess,
    V: BinaryValue + ObjectHash,
{
    type Item = (Hash, V);
    type IntoIter = Iter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for Iter<'a, V>
where
    V: BinaryValue + ObjectHash,
{
    type Item = (Hash, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

impl<'a> Iterator for Hashes<'a> {
    type Item = Hash;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

#[cfg(test)]
mod tests {
    use crate::{access::AccessExt, Database, ObjectHash, TemporaryDB};

    #[test]
    fn value_set_methods() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut index = fork.get_value_set("index");

        assert!(!index.contains(&1_u8));
        assert!(!index.contains_by_hash(&1_u8.object_hash()));

        index.insert(1_u8);
        assert!(index.contains(&1_u8));
        assert!(index.contains_by_hash(&1_u8.object_hash()));

        index.insert(2_u8);
        let hash = index.hashes().next().unwrap();
        index.remove_by_hash(&hash);
        assert!(!index.contains(&1_u8));

        index.clear();
        assert!(!index.contains(&2_u8));
    }
}
