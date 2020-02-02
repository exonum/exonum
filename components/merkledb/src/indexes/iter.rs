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

/// FIXME
#[derive(Debug)]
pub struct Entries<'a, K: ?Sized, V> {
    base_iter: Iter<'a, K, V>,
}

impl<'a, K, V> Entries<'a, K, V>
where
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    pub(crate) fn new<T: RawAccess>(view: &'a View<T>, from: Option<&K>) -> Self {
        Self::with_prefix(view, &(), from)
    }

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

    /// FIXME
    pub fn skip_values(self) -> Keys<'a, K> {
        Keys {
            base_iter: self.base_iter.drop_value_type(),
        }
    }

    /// FIXME
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

/// FIXME
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

/// FIXME
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
