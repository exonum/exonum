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

#![warn(missing_docs)]

use failure::bail;

pub use self::metadata::{BinaryAttribute, IndexState, IndexType};

use std::{borrow::Cow, fmt, iter::Peekable, marker::PhantomData};

use super::{
    db::{Change, ChangesRef, ForkIter, ViewChanges},
    BinaryKey, BinaryValue, Iter as BytesIter, Iterator as BytesIterator, Snapshot,
};

mod metadata;
mod refs;
#[cfg(test)]
mod tests;

/// Base view struct responsible for accessing indexes.
// TODO: add documentation [ECR-2820]
pub struct View<T: IndexAccess> {
    pub address: IndexAddress,
    pub index_access: T,
    pub changes: T::Changes,
}

impl<T: IndexAccess> fmt::Debug for View<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("View")
            .field("address", &self.address)
            .finish()
    }
}

/// TODO: add documentation [ECR-2820]
pub trait ChangeSet {
    /// TODO: add documentation [ECR-2820]
    fn as_ref(&self) -> Option<&ViewChanges>;
    /// TODO: add documentation [ECR-2820]
    fn as_mut(&mut self) -> Option<&mut ViewChanges>;
}

impl ChangeSet for () {
    fn as_ref(&self) -> Option<&ViewChanges> {
        None
    }
    fn as_mut(&mut self) -> Option<&mut ViewChanges> {
        None
    }
}

impl ChangeSet for ChangesRef<'_> {
    fn as_ref(&self) -> Option<&ViewChanges> {
        Some(&*self)
    }
    fn as_mut(&mut self) -> Option<&mut ViewChanges> {
        Some(&mut *self)
    }
}

/// TODO: add documentation [ECR-2820]
pub trait IndexAccess: Copy {
    /// TODO: add documentation [ECR-2820]
    type Changes: ChangeSet;
    /// TODO: add documentation [ECR-2820]
    fn snapshot(&self) -> &dyn Snapshot;
    /// TODO: add documentation [ECR-2820]
    fn changes(&self, address: &IndexAddress) -> Self::Changes;
}

/// Struct responsible for creating indexes from `view` with
/// specified `address`.
// TODO: add documentation [ECR-2820]
#[derive(Debug)]
pub struct IndexBuilder<T> {
    index_access: T,
    address: IndexAddress,
    index_type: IndexType,
}

impl<T> IndexBuilder<T>
where
    T: IndexAccess,
{
    /// Create index from `view'.
    pub fn new(index_access: T) -> Self {
        let address = IndexAddress::default();
        Self {
            index_access,
            address,
            index_type: IndexType::default(),
        }
    }

    ///TODO: add docs
    pub fn from_view(view: View<T>) -> Self {
        Self {
            address: view.address,
            index_access: view.index_access,
            index_type: IndexType::default(),
        }
    }

    ///TODO: add docs
    pub fn from_address<I: Into<IndexAddress>>(address: I, index_access: T) -> Self {
        Self {
            index_access,
            address: address.into(),
            index_type: IndexType::default(),
        }
    }

    /// Provides first part of the index address.
    pub fn index_name<S: Into<String>>(self, index_name: S) -> Self {
        let address = self.address.append_name(index_name.into());
        Self {
            index_access: self.index_access,
            address,
            index_type: self.index_type,
        }
    }

    /// Provides `family_id` for the index address.
    pub fn family_id<I>(self, family_id: &I) -> Self
    where
        I: BinaryKey + ?Sized,
    {
        let address = self.address.append_bytes(family_id);
        Self {
            index_access: self.index_access,
            address,
            index_type: self.index_type,
        }
    }

    /// Sets the type of the given index.
    pub fn index_type(self, index_type: IndexType) -> Self {
        Self {
            index_access: self.index_access,
            address: self.address,
            index_type,
        }
    }

    fn create_state<V>(self) -> (View<T>, IndexState<T, V>)
    where
        V: BinaryAttribute + Default + Copy,
    {
        // TODO Think about stricter restrictions for index names. [ECR-2834]
        assert!(
            !self.address.name().is_empty(),
            "Index name must not be empty"
        );

        let (index_address, index_state) =
            metadata::index_metadata(self.index_access, &self.address, self.index_type);
        let index_view = View::new(self.index_access, index_address);

        (index_view, index_state)
    }

    /// Returns index that builds upon specified `view` and `address`.
    ///
    /// # Panics
    ///
    /// - Panics if index metadata doesn't match expected.
    /// - Panics if index name is empty.
    pub fn build<V>(self) -> (View<T>, IndexState<T, V>)
    where
        V: BinaryAttribute + Default + Copy,
    {
        self.create_state()
    }

    ///TODO: add docs
    pub fn build_new<V>(self) -> Result<(View<T>, IndexState<T, V>), failure::Error>
    where
        V: BinaryAttribute + Default + Copy,
    {
        let (index_view, index_state) = self.create_state();

        if !index_state.is_new() {
            bail!("Index is already exists!");
        }

        Ok((index_view, index_state))
    }

    ///TODO: add docs
    pub fn build_existed<V>(self) -> Result<(View<T>, IndexState<T, V>), failure::Error>
    where
        V: BinaryAttribute + Default + Copy,
    {
        let (index_view, index_state) = self.create_state();

        if index_state.is_new() {
            bail!("Index is not found!");
        }

        Ok((index_view, index_state))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct IndexAddress {
    pub(super) name: String,
    pub(super) bytes: Option<Vec<u8>>,
}

impl IndexAddress {
    /// TODO: add documentation [ECR-2820]
    pub fn new() -> Self {
        Self::default()
    }

    /// TODO: add documentation [ECR-2820]
    pub fn with_root<S: Into<String>>(root: S) -> Self {
        Self {
            name: root.into(),
            bytes: None,
        }
    }

    /// TODO: add documentation [ECR-2820]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// TODO: add documentation [ECR-2820]
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_ref().map(Vec::as_slice)
    }

    /// TODO: add documentation [ECR-2820]
    pub fn keyed<'a>(&self, key: &'a [u8]) -> (&str, Cow<'a, [u8]>) {
        (
            &self.name,
            match self.bytes {
                None => Cow::Borrowed(key),
                Some(ref bytes) => {
                    let bytes = concat_keys!(bytes, key);
                    bytes.into()
                }
            },
        )
    }

    /// TODO: add documentation [ECR-2820]
    pub fn append_name<'a, S: Into<Cow<'a, str>>>(self, suffix: S) -> Self {
        let suffix = suffix.into();
        Self {
            name: if self.name.is_empty() {
                suffix.into_owned()
            } else {
                // Because `concat` is faster than `format!("...")` in all cases.
                [self.name(), ".", suffix.as_ref()].concat()
            },

            bytes: self.bytes,
        }
    }

    /// TODO: add documentation [ECR-2820]
    pub fn append_bytes<K: BinaryKey + ?Sized>(self, suffix: &K) -> Self {
        let name = self.name;
        let bytes = if let Some(bytes) = self.bytes {
            concat_keys!(bytes, suffix)
        } else {
            concat_keys!(suffix)
        };

        Self {
            name,
            bytes: Some(bytes),
        }
    }
}

impl<'a> From<&'a str> for IndexAddress {
    fn from(name: &'a str) -> Self {
        Self::with_root(name)
    }
}

impl From<String> for IndexAddress {
    fn from(name: String) -> Self {
        Self::with_root(name)
    }
}

/// TODO should we have this impl in public interface? ECR-2834
impl<'a, K: BinaryKey + ?Sized> From<(&'a str, &'a K)> for IndexAddress {
    fn from((name, key): (&'a str, &'a K)) -> Self {
        Self {
            name: name.to_owned(),
            bytes: Some(key_bytes(key)),
        }
    }
}

impl<'a> IndexAccess for &'a dyn Snapshot {
    type Changes = ();

    fn snapshot(&self) -> &dyn Snapshot {
        *self
    }

    fn changes(&self, _: &IndexAddress) -> Self::Changes {}
}

impl<'a> IndexAccess for &'a Box<dyn Snapshot> {
    type Changes = ();

    fn snapshot(&self) -> &dyn Snapshot {
        self.as_ref()
    }

    fn changes(&self, _: &IndexAddress) -> Self::Changes {}
}

//impl fmt::Debug for IndexAddress {
//    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//        f.debug_tuple("IndexAddress").field(&self.name).field(&self.bytes).finish()
//    }
//}

fn key_bytes<K: BinaryKey + ?Sized>(key: &K) -> Vec<u8> {
    concat_keys!(key)
}

impl<T: IndexAccess> View<T> {
    pub(super) fn new<I: Into<IndexAddress>>(index_access: T, address: I) -> Self {
        let address = address.into();
        let changes = index_access.changes(&address);
        Self {
            index_access,
            changes,
            address,
        }
    }

    fn snapshot(&self) -> &dyn Snapshot {
        self.index_access.snapshot()
    }

    fn get_bytes(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(ref changes) = self.changes.as_ref() {
            if let Some(change) = changes.data.get(key) {
                match *change {
                    Change::Put(ref v) => return Some(v.clone()),
                    Change::Delete => return None,
                }
            }

            if changes.is_empty() {
                return None;
            }
        }

        let (name, key) = self.address.keyed(key);
        self.snapshot().get(name, &key)
    }

    fn contains_raw_key(&self, key: &[u8]) -> bool {
        if let Some(ref changes) = self.changes.as_ref() {
            if let Some(change) = changes.data.get(key) {
                match *change {
                    Change::Put(..) => return true,
                    Change::Delete => return false,
                }
            }

            if changes.is_empty() {
                return false;
            }
        }

        let (name, key) = self.address.keyed(key);
        self.snapshot().contains(name, &key)
    }

    fn iter_bytes(&self, from: &[u8]) -> BytesIter {
        use std::collections::Bound::*;

        let (name, key) = self.address.keyed(from);
        let prefix = self.address.bytes.clone().unwrap_or_else(|| vec![]);

        let changes_iter = self
            .changes
            .as_ref()
            .map(|changes| changes.data.range::<[u8], _>((Included(from), Unbounded)));

        let is_empty = self
            .changes
            .as_ref()
            .map_or(false, |changes| changes.is_empty());

        if is_empty {
            // Ignore all changes from the snapshot
            Box::new(ChangesIter::new(changes_iter.unwrap()))
        } else {
            Box::new(ForkIter::new(
                Box::new(SnapshotIter::new(self.snapshot(), name, prefix, &key)),
                changes_iter,
            ))
        }
    }

    /// Returns a value of *any* type corresponding to the key of *any* type.
    pub fn get<K, V>(&self, key: &K) -> Option<V>
    where
        K: BinaryKey + ?Sized,
        V: BinaryValue,
    {
        self.get_bytes(&key_bytes(key)).map(|v| {
            BinaryValue::from_bytes(Cow::Owned(v)).expect("Error while deserializing value")
        })
    }

    /// Returns `true` if the index contains a value of *any* type for the specified key of
    /// *any* type.
    pub fn contains<K>(&self, key: &K) -> bool
    where
        K: BinaryKey + ?Sized,
    {
        self.contains_raw_key(&key_bytes(key))
    }

    /// Returns an iterator over the entries of the index in ascending order. The iterator element
    /// type is *any* key-value pair. An argument `subprefix` allows specifying a subset of keys
    /// for iteration.
    pub fn iter<P, K, V>(&self, subprefix: &P) -> Iter<K, V>
    where
        P: BinaryKey + ?Sized,
        K: BinaryKey,
        V: BinaryValue,
    {
        let iter_prefix = key_bytes(subprefix);
        Iter {
            base_iter: self.iter_bytes(&iter_prefix),
            prefix: iter_prefix,
            ended: false,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Returns an iterator over the entries of the index in ascending order starting from the
    /// specified key. The iterator element type is *any* key-value pair. An argument `subprefix`
    /// allows specifying a subset of iteration.
    pub fn iter_from<P, F, K, V>(&self, subprefix: &P, from: &F) -> Iter<K, V>
    where
        P: BinaryKey,
        F: BinaryKey + ?Sized,
        K: BinaryKey,
        V: BinaryValue,
    {
        let iter_prefix = key_bytes(subprefix);
        let iter_from = key_bytes(from);
        Iter {
            base_iter: self.iter_bytes(&iter_from),
            prefix: iter_prefix,
            ended: false,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Inserts a key-value pair into the fork.
    pub fn put<K, V>(&mut self, key: &K, value: V)
    where
        K: BinaryKey + ?Sized,
        V: BinaryValue,
    {
        if let Some(changes) = self.changes.as_mut() {
            changes
                .data
                .insert(concat_keys!(key), Change::Put(value.into_bytes()));
        };
    }

    /// Removes a key from the view.
    pub fn remove<K>(&mut self, key: &K)
    where
        K: BinaryKey + ?Sized,
    {
        if let Some(changes) = self.changes.as_mut() {
            changes.data.insert(concat_keys!(key), Change::Delete);
        };
    }

    /// Clears the view removing all its elements.
    pub fn clear(&mut self) {
        if let Some(changes) = self.changes.as_mut() {
            changes.clear()
        }
    }
}

/// Iterator over entries in a snapshot limited to a specific view.
struct SnapshotIter<'a> {
    inner: BytesIter<'a>,
    prefix: Vec<u8>,
    ended: bool,
}

impl<'a> fmt::Debug for SnapshotIter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SnapshotIter")
            .field("prefix", &self.prefix)
            .field("ended", &self.ended)
            .finish()
    }
}

impl<'a> SnapshotIter<'a> {
    fn new(snapshot: &'a dyn Snapshot, name: &str, prefix: Vec<u8>, from: &[u8]) -> Self {
        debug_assert!(from.starts_with(&prefix));

        SnapshotIter {
            inner: snapshot.iter(name, from),
            prefix,
            ended: false,
        }
    }
}

impl<'a> BytesIterator for SnapshotIter<'a> {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        if self.ended {
            return None;
        }

        let next = self.inner.next();
        match next {
            Some((k, v)) if k.starts_with(&self.prefix) => Some((&k[self.prefix.len()..], v)),
            _ => {
                self.ended = true;
                None
            }
        }
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        if self.ended {
            return None;
        }

        let peeked = self.inner.peek();
        match peeked {
            Some((k, v)) if k.starts_with(&self.prefix) => Some((&k[self.prefix.len()..], v)),
            _ => {
                self.ended = true;
                None
            }
        }
    }
}

struct ChangesIter<'a, T: Iterator + 'a> {
    inner: Peekable<T>,
    _lifetime: PhantomData<&'a ()>,
}

/// Iterator over a set of changes.
impl<'a, T> ChangesIter<'a, T>
where
    T: Iterator<Item = (&'a Vec<u8>, &'a Change)>,
{
    fn new(iterator: T) -> Self {
        ChangesIter {
            inner: iterator.peekable(),
            _lifetime: PhantomData,
        }
    }
}

impl<'a, T> BytesIterator for ChangesIter<'a, T>
where
    T: Iterator<Item = (&'a Vec<u8>, &'a Change)>,
{
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            match self.inner.next() {
                Some((key, &Change::Put(ref value))) => {
                    return Some((key.as_slice(), value.as_slice()));
                }
                Some((_, &Change::Delete)) => {}
                None => {
                    return None;
                }
            }
        }
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            match self.inner.peek() {
                Some((key, &Change::Put(ref value))) => {
                    return Some((key.as_slice(), value.as_slice()));
                }
                Some((_, &Change::Delete)) => {}
                None => {
                    return None;
                }
            }
        }
    }
}

/// An iterator over the entries of a `View`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] method on [`View`]. See its documentation for details.
///
/// [`iter`]: struct.BaseIndex.html#method.iter
/// [`iter_from`]: struct.BaseIndex.html#method.iter_from
/// [`BaseIndex`]: struct.BaseIndex.html
pub struct Iter<'a, K, V> {
    base_iter: BytesIter<'a>,
    prefix: Vec<u8>,
    ended: bool,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<'a, K, V> fmt::Debug for Iter<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Iter(..)")
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: BinaryKey,
    V: BinaryValue,
{
    type Item = (K::Owned, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }

        if let Some((k, v)) = self.base_iter.next() {
            if k.starts_with(&self.prefix) {
                return Some((
                    K::read(k),
                    V::from_bytes(Cow::Borrowed(v))
                        .expect("Unable to decode value from bytes, an error occurred"),
                ));
            }
        }

        self.ended = true;
        None
    }
}
