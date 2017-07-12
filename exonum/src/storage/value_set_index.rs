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

//! An implementation of set for items that implement `StorageValue` trait.
use std::marker::PhantomData;

use crypto::Hash;

use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageValue};

/// A set of items that implement `StorageValue` trait.
///
/// `ValueSetIndex` implements a set, storing the element as values using its hash as a key.
/// `ValueSetIndex` requires that the elements implement the [`StorageValue`] trait.
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

impl<T, V> ValueSetIndex<T, V> {
    /// Creates a new index representation based on the common prefix of its keys and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    pub fn new(prefix: Vec<u8>, view: T) -> Self {
        ValueSetIndex {
            base: BaseIndex::new(prefix, view),
            _v: PhantomData,
        }
    }
}

impl<T, V> ValueSetIndex<T, V>
where
    T: AsRef<Snapshot>,
    V: StorageValue,
{
    /// Returns `true` if the set contains a value.
    pub fn contains(&self, item: &V) -> bool {
        self.contains_by_hash(&item.hash())
    }

    /// Returns `true` if the set contains a value with the specified hash.
    pub fn contains_by_hash(&self, hash: &Hash) -> bool {
        self.base.contains(hash)
    }

    /// An iterator visiting all elements in arbitrary order. The iterator element type is V.
    pub fn iter(&self) -> ValueSetIndexIter<V> {
        ValueSetIndexIter { base_iter: self.base.iter(&()) }
    }

    /// An iterator visiting all elements in arbitrary order starting from the specified hash of
    /// a value. The iterator element type is V.
    pub fn iter_from(&self, from: &Hash) -> ValueSetIndexIter<V> {
        ValueSetIndexIter { base_iter: self.base.iter_from(&(), from) }
    }

    /// An iterator visiting hashes of all elements in ascending order. The iterator element type
    /// is [Hash](../../crypto/struct.Hash.html).
    pub fn hashes(&self) -> ValueSetIndexHashes {
        ValueSetIndexHashes { base_iter: self.base.iter(&()) }
    }

    /// An iterator visiting hashes of all elements in ascending order starting from the specified
    /// hash. The iterator element type is [Hash](../../crypto/struct.Hash.html).
    pub fn hashes_from(&self, from: &Hash) -> ValueSetIndexHashes {
        ValueSetIndexHashes { base_iter: self.base.iter_from(&(), from) }
    }
}

impl<'a, V> ValueSetIndex<&'a mut Fork, V>
where
    V: StorageValue,
{
    /// Adds a value to the set.
    pub fn insert(&mut self, item: V) {
        self.base.put(&item.hash(), item)
    }

    /// Removes a value from the set.
    pub fn remove(&mut self, item: &V) {
        self.remove_by_hash(&item.hash())
    }

    /// Removes a value from the set by the specified hash.
    pub fn remove_by_hash(&mut self, hash: &Hash) {
        self.base.remove(hash)
    }

    /// Clears the set, removing all values.
    ///
    /// # Notes
    /// Currently this method is not optimized to delete large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
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
