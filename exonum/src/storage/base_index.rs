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

//! An implementation of base index with most common features.
use std::sync::Arc;
use std::borrow::Cow;
use std::marker::PhantomData;

use super::{StorageKey, StorageValue, View, Iter};

/// Basic struct for all indices that implements common features.
///
/// This structure is not intended for direct use, rather it is the basis for building other types
/// of indices.
///
/// `BaseIndex` requires that the keys implement the [`StorageKey`] trait and the values implement
/// [`StorageValue`] trait. However, this structure is not bound to specific types and allows the
/// use of *any* types as keys or values.
/// [`StorageKey`]: ../trait.StorageKey.html
/// [`StorageValue`]: ../trait.StorageValue.html
#[derive(Debug)]
pub struct BaseIndex {
    name: String,
    view: Arc<View>,
}

/// An iterator over the entries of a `BaseIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`BaseIndex`]. See its documentation for more.
///
/// [`iter`]: struct.BaseIndex.html#method.iter
/// [`iter_from`]: struct.BaseIndex.html#method.iter_from
/// [`BaseIndex`]: struct.BaseIndex.html
pub struct BaseIndexIter<K, V> {
    base_iter: Iter,
    ended: bool,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl BaseIndex {
    /// Creates a new index representation based on the common prefix of its keys and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    pub fn new(name: &str, view: Arc<View>) -> Self {

        BaseIndex {
            name: name.to_string(),
            view,
        }
    }
}

fn gen_key<K: StorageKey>(key: &K) -> Vec<u8> {
    let mut v = vec![0; key.size()];
    key.write(&mut v);
    v
}

impl BaseIndex {
    /// Returns a value of *any* type corresponding to the key of *any* type.
    pub fn get<K, V>(&self, key: &K) -> Option<V>
    where
        K: StorageKey,
        V: StorageValue,
    {
        self.view.get(&self.name, &gen_key(key)).map(|v| {
            StorageValue::from_bytes(Cow::Owned(v))
        })
    }

    /// Returns `true` if the index contains a value of *any* type for the specified key of
    /// *any* type.
    pub fn contains<K>(&self, key: &K) -> bool
    where
        K: StorageKey,
    {
        self.view.contains(&self.name, &gen_key(key))
    }

    /// Returns an iterator over the entries of the index in ascending order. The iterator element
    /// type is *any* key-value pair. An argument `subprefix` allows to specify a subset of keys
    /// for iteration.
    pub fn iter<K, V>(&self) -> BaseIndexIter<K, V>
    where
        K: StorageKey,
        V: StorageValue,
    {
        BaseIndexIter {
            base_iter: self.view.iter(&self.name, None),
            ended: false,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Returns an iterator over the entries of the index in ascending order starting from the
    /// specified key. The iterator element type is *any* key-value pair. An argument `subprefix`
    /// allows to specify a subset of iteration.
    pub fn iter_from<F, K, V>(&self, from: &F) -> BaseIndexIter<K, V>
    where
        K: StorageKey,
        F: StorageKey,
        V: StorageValue,
    {
        BaseIndexIter {
            base_iter: self.view.iter(&self.name, Some(&gen_key(from))),
            ended: false,
            _k: PhantomData,
            _v: PhantomData,
        }
    }
    //}
    //
    //impl<'a>BaseIndex<&'a mut View> {
    /// Inserts the key-value pair into the index. Both key and value may be of *any* types.
    pub fn put<K, V>(&mut self, key: &K, value: V)
    where
        K: StorageKey,
        V: StorageValue,
    {
        self.view.put(
            &self.name,
            &gen_key(key),
            value.into_bytes().as_ref(),
        );
    }

    /// Removes the key of *any* type from the index.
    pub fn remove<K>(&mut self, key: &K)
    where
        K: StorageKey,
    {
        self.view.delete(&self.name, &gen_key(key));
    }

    /// Clears the index, removing all entries.
    ///
    /// # Notes
    ///
    /// Currently this method is not optimized to delete large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    pub fn clear(&mut self) {
        self.view.clear(&self.name)
    }
}

impl<K, V> Iterator for BaseIndexIter<K, V>
where
    K: StorageKey,
    V: StorageValue,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        if let Some((k, v)) = self.base_iter.next() {
            Some((K::read(&k[..]), V::from_bytes(Cow::Borrowed(v))))
        } else {
            self.ended = true;
            None
        }
    }
}

impl<K, V> ::std::fmt::Debug for BaseIndexIter<K, V> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "BaseIndexIter(..)")
    }
}
