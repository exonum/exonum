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

//! An implementation of base index with most common features.
//!
//! The `BaseIndex` structure is not intended for direct use, rather it is the
//! basis for building other types of indices. The given section contains methods
//! related to `BaseIndex` and the iterator over the items of this index.

// spell-checker:ignore subprefix

use std::marker::PhantomData;

use super::{BinaryForm, Fork, Iter, Snapshot, StorageKey};
use crate::indexes_metadata::{self, IndexType, INDEXES_METADATA_TABLE_NAME};

/// Basic struct for all indices that implements common features.
///
/// This structure is not intended for direct use, rather it is the basis for building other types
/// of indices.
///
/// `BaseIndex` requires that keys should implement the [`StorageKey`] trait and
/// values should implement the [`BinaryForm`] trait. However, this structure
/// is not bound to specific types and allows the use of *any* types as keys or values.
///
/// [`StorageKey`]: ../trait.StorageKey.html
/// [`BinaryForm`]: ../trait.BinaryForm.html
#[derive(Debug)]
pub struct BaseIndex<T> {
    name: String,
    is_family: bool,
    index_id: Option<Vec<u8>>,
    is_mutable: bool,
    index_type: IndexType,
    view: T,
}

/// An iterator over the entries of a `BaseIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] method on [`BaseIndex`]. See its documentation for details.
///
/// [`iter`]: struct.BaseIndex.html#method.iter
/// [`iter_from`]: struct.BaseIndex.html#method.iter_from
/// [`BaseIndex`]: struct.BaseIndex.html
pub struct BaseIndexIter<'a, K, V> {
    base_iter: Iter<'a>,
    base_prefix_len: usize,
    index_id: Vec<u8>,
    ended: bool,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<T> BaseIndex<T>
where
    T: AsRef<dyn Snapshot>,
{
    /// Creates a new index representation based on the name and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case, only
    /// immutable methods are available. In the second case, both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    pub fn new<S: AsRef<str>>(index_name: S, index_type: IndexType, view: T) -> Self {
        assert_valid_name(&index_name);

        let is_family = false;
        indexes_metadata::assert_index_type(
            index_name.as_ref(),
            index_type,
            is_family,
            view.as_ref(),
        );

        Self {
            name: index_name.as_ref().to_string(),
            is_family,
            index_id: None,
            is_mutable: false,
            index_type,
            view,
        }
    }

    /// Creates a new index representation based on the family name, index ID inside family
    /// and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case, only
    /// immutable methods are available. In the second case, both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    pub fn new_in_family<S, P>(family_name: S, index_id: &P, index_type: IndexType, view: T) -> Self
    where
        P: StorageKey,
        P: ?Sized,
        S: AsRef<str>,
    {
        assert_valid_name(&family_name);

        let is_family = true;
        indexes_metadata::assert_index_type(
            family_name.as_ref(),
            index_type,
            is_family,
            view.as_ref(),
        );

        Self {
            name: family_name.as_ref().to_string(),
            is_family,
            index_id: {
                let mut buf = vec![0; index_id.size()];
                index_id.write(&mut buf);
                Some(buf)
            },
            is_mutable: false,
            index_type,
            view,
        }
    }

    pub(crate) fn indexes_metadata(view: T) -> Self {
        Self {
            name: INDEXES_METADATA_TABLE_NAME.to_string(),
            is_family: false,
            index_id: None,
            is_mutable: true,
            index_type: IndexType::Map,
            view,
        }
    }

    fn prefixed_key<K: StorageKey + ?Sized>(&self, key: &K) -> Vec<u8> {
        if let Some(ref prefix) = self.index_id {
            let mut v = vec![0; prefix.len() + key.size()];
            v[..prefix.len()].copy_from_slice(prefix);
            key.write(&mut v[prefix.len()..]);
            v
        } else {
            let mut v = vec![0; key.size()];
            key.write(&mut v);
            v
        }
    }

    /// Returns a value of *any* type corresponding to the key of *any* type.
    pub fn get<K, V>(&self, key: &K) -> Option<V>
    where
        K: StorageKey + ?Sized,
        V: BinaryForm,
    {
        self.view
            .as_ref()
            .get(&self.name, &self.prefixed_key(key))
            .map(|v| BinaryForm::from_bytes(v.into()).expect("Unable to decode value"))
    }

    /// Returns `true` if the index contains a value of *any* type for the specified key of
    /// *any* type.
    pub fn contains<K>(&self, key: &K) -> bool
    where
        K: StorageKey + ?Sized,
    {
        self.view
            .as_ref()
            .contains(&self.name, &self.prefixed_key(key))
    }

    /// Returns an iterator over the entries of the index in ascending order. The iterator element
    /// type is *any* key-value pair. An argument `subprefix` allows specifying a subset of keys
    /// for iteration.
    pub fn iter<P, K, V>(&self, subprefix: &P) -> BaseIndexIter<K, V>
    where
        P: StorageKey,
        K: StorageKey,
        V: BinaryForm,
    {
        let iter_prefix = self.prefixed_key(subprefix);
        BaseIndexIter {
            base_iter: self.view.as_ref().iter(&self.name, &iter_prefix),
            base_prefix_len: self.index_id.as_ref().map_or(0, |p| p.len()),
            index_id: iter_prefix,
            ended: false,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Returns an iterator over the entries of the index in ascending order starting from the
    /// specified key. The iterator element type is *any* key-value pair. An argument `subprefix`
    /// allows specifying a subset of iteration.
    pub fn iter_from<P, F, K, V>(&self, subprefix: &P, from: &F) -> BaseIndexIter<K, V>
    where
        P: StorageKey,
        F: StorageKey + ?Sized,
        K: StorageKey,
        V: BinaryForm,
    {
        let iter_prefix = self.prefixed_key(subprefix);
        let iter_from = self.prefixed_key(from);
        BaseIndexIter {
            base_iter: self.view.as_ref().iter(&self.name, &iter_from),
            base_prefix_len: self.index_id.as_ref().map_or(0, |p| p.len()),
            index_id: iter_prefix,
            ended: false,
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<'a> BaseIndex<&'a mut Fork> {
    fn set_index_type(&mut self) {
        if !self.is_mutable {
            indexes_metadata::set_index_type(
                &self.name,
                self.index_type,
                self.is_family,
                &mut self.view,
            );
            self.is_mutable = true;
        }
    }

    /// Inserts the key-value pair into the index. Both key and value may be of *any* types.
    pub fn put<K, V>(&mut self, key: &K, value: V)
    where
        K: StorageKey,
        V: BinaryForm,
    {
        self.set_index_type();
        let key = self.prefixed_key(key);
        self.view.put(&self.name, key, value.to_bytes());
    }

    /// Removes the key of *any* type from the index.
    pub fn remove<K>(&mut self, key: &K)
    where
        K: StorageKey + ?Sized,
    {
        self.set_index_type();
        let key = self.prefixed_key(key);
        self.view.remove(&self.name, key);
    }

    /// Clears the index, removing entries with keys that start with a prefix or all entries
    /// if `prefix` is `None`.
    ///
    /// # Notes
    ///
    /// Currently this method is not optimized to delete a large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    pub fn clear(&mut self) {
        self.set_index_type();
        let prefix = self.index_id.as_ref().map(Vec::as_slice);
        self.view.remove_by_prefix(&self.name, prefix);
    }
}

impl<'a, K, V> Iterator for BaseIndexIter<'a, K, V>
where
    K: StorageKey,
    V: BinaryForm,
{
    type Item = (K::Owned, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        if let Some((k, v)) = self.base_iter.next() {
            if k.starts_with(&self.index_id) {
                return Some((
                    K::read(&k[self.base_prefix_len..]),
                    V::from_bytes(v.into()).expect("Unable to decode value"),
                ));
            }
        }
        self.ended = true;
        None
    }
}

impl<'a, K, V> ::std::fmt::Debug for BaseIndexIter<'a, K, V> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "BaseIndexIter(..)")
    }
}

/// A function that validates an index name. Allowable characters in name: ASCII characters, digits
/// and underscores.
fn is_valid_name<S: AsRef<str>>(name: S) -> bool {
    name.as_ref().as_bytes().iter().all(|c| match *c {
        48...57 | 65...90 | 97...122 | 95 | 46 => true,
        _ => false,
    })
}

/// Calls the `is_valid_name` function with the given name and panics if it returns `false`.
fn assert_valid_name<S: AsRef<str>>(name: S) {
    if !is_valid_name(name) {
        panic!("Wrong characters using in name. Use: a-zA-Z0-9 and _");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_name_validator() {
        // spell-checker:disable
        assert!(is_valid_name("index_name"));
        assert!(is_valid_name("_index_name"));
        assert!(is_valid_name("AinDex_name_"));
        assert!(is_valid_name("core.index_name1Z"));
        assert!(is_valid_name("configuration.indeX_1namE"));
        assert!(is_valid_name("1index_Namez"));

        assert!(!is_valid_name("index-name"));
        assert!(!is_valid_name("_index-name"));
        assert!(!is_valid_name("индекс_name_"));
        assert!(!is_valid_name("core.index_имя3"));
        assert!(!is_valid_name("indeX_1namE-"));
        assert!(!is_valid_name("1in!dex_Namez"));
    }

    #[test]
    fn test_check_valid_name() {
        assert_valid_name("valid_name");
    }

    #[test]
    #[should_panic(expected = "Wrong characters using in name. Use: a-zA-Z0-9 and _")]
    fn test_check_invalid_name() {
        assert_valid_name("invalid-name");
    }
}
