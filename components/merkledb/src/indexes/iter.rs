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

//! Generic iterator types used by all indexes.

use crate::{
    views::{Iter, RawAccess, View},
    BinaryKey, BinaryValue,
};

/// Iterator over key-value pairs of an index.
///
/// This structure is returned by the [`IndexIterator`] trait and by inherent methods
/// of some indexes.
///
/// [`IndexIterator`]: trait.IndexIterator.html
#[derive(Debug)]
pub struct Entries<'a, K: ?Sized, V> {
    base_iter: Iter<'a, K, V>,
}

impl<'a, K, V> Entries<'a, K, V>
where
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    /// Creates a new iterator based on the provided view.
    pub(crate) fn new<T: RawAccess>(view: &'a View<T>, from: Option<&K>) -> Self {
        Self::with_prefix(view, &(), from)
    }

    /// Creates a new iterator based on the provided view. The keys returned by the iterator
    /// are additionally filtered by the `prefix`.
    pub(crate) fn with_prefix<T, P>(view: &'a View<T>, prefix: &P, from: Option<&K>) -> Self
    where
        T: RawAccess,
        P: BinaryKey,
    {
        let base_iter = if let Some(from) = from {
            view.iter_from(prefix, from)
        } else {
            view.iter(prefix)
        };
        Self { base_iter }
    }

    /// Creates a new iterator based on the provided view. The keys returned by the iterator
    /// are additionally filtered by the `prefix`, which is detached from the key before
    /// deserialization.
    pub(crate) fn with_detached_prefix<T, P>(
        view: &'a View<T>,
        prefix: &P,
        from: Option<&K>,
    ) -> Self
    where
        T: RawAccess,
        P: BinaryKey,
    {
        let base_iter = view.iter_detached(prefix, from);
        Self { base_iter }
    }

    /// Skips values in the iterator output without parsing them.
    pub fn skip_values(self) -> Keys<'a, K> {
        Keys {
            base_iter: self.base_iter.drop_value_type(),
        }
    }

    /// Skips keys in the iterator output without parsing them.
    pub fn skip_keys(self) -> Values<'a, V> {
        Values {
            base_iter: self.base_iter.drop_key_type(),
        }
    }
}

impl<K, V> Iterator for Entries<'_, K, V>
where
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    type Item = (K::Owned, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

/// Iterator over keys of an index.
///
/// This structure is returned by [`Entries::skip_values`] , and by inherent methods
/// of some indexes.
///
/// [`Entries::skip_values`]: struct.Entries.html#method.skip_values
#[derive(Debug)]
pub struct Keys<'a, K: ?Sized> {
    base_iter: Iter<'a, K, ()>,
}

impl<K> Iterator for Keys<'_, K>
where
    K: BinaryKey + ?Sized,
{
    type Item = K::Owned;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(key, _)| key)
    }
}

/// Iterator over values of an index.
///
/// This structure is returned by [`Entries::skip_keys`] , and by inherent methods
/// of some indexes.
///
/// [`Entries::skip_keys`]: struct.Entries.html#method.skip_keys
#[derive(Debug)]
pub struct Values<'a, V> {
    base_iter: Iter<'a, (), V>,
}

impl<V> Iterator for Values<'_, V>
where
    V: BinaryValue,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(_, value)| value)
    }
}

/// Database object that supports iteration and continuing iteration from an intermediate position.
///
/// This trait is implemented for all index collections (i.e., all index types except for
/// `Entry` and `ProofEntry`) and can thus be used by the generic iteration routines.
pub trait IndexIterator {
    /// Type encompassing index keys.
    type Key: BinaryKey + ?Sized;
    /// Type encompassing index values.
    type Value: BinaryValue;

    /// Continues iteration from the specified position. If `from` is `None`, starts the iteration
    /// from scratch.
    fn index_iter(&self, from: Option<&Self::Key>) -> Entries<'_, Self::Key, Self::Value>;
}
