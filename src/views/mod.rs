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

#![allow(missing_docs)]

use std::{borrow::Cow, fmt, iter::Peekable, marker::PhantomData, cell::{RefMut, Ref}};

use super::{
    db::{Change, ForkIter, ViewChanges}, Iter as BytesIter, Iterator as BytesIterator, Snapshot,
    BinaryKey, BinaryValue,
};
use exonum_crypto::Hash;

//#[cfg(test)]
//mod tests;

/// Base view struct responsible for accessing indexes.
pub struct View<'a> {
    snapshot: &'a dyn Snapshot,
    address: IndexAddress,
    changes: Option<Ref<'a, ViewChanges>>,
}

impl<'a> fmt::Debug for View<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("View")
            .field("address", &self.address)
            .finish()
    }
}

impl<'a> View<'a> {
    pub(super) fn new(
        snapshot: &'a dyn Snapshot,
        address: IndexAddress,
        changes: Ref<'a, ViewChanges>,
    ) -> Self {
        View {
            snapshot,
            address,
            changes: Some(changes),
        }
    }
}

/// TODO
pub struct ViewMut<'a> {
    snapshot: &'a dyn Snapshot,
    address: IndexAddress,
    changes: RefMut<'a, ViewChanges>,
}

impl<'a> fmt::Debug for ViewMut<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ViewMut")
            .field("address", &self.address)
            .finish()
    }
}

pub trait IndexAccess {
    fn view<I: Into<IndexAddress>>(&self, address: I) -> View;

    fn index<'a, I, T>(&'a self, address: I) -> T
        where
            I: Into<IndexAddress>,
            T: FromView<View<'a>>,
    {
        T::from_view(self.view(address))
    }
}

pub trait IndexAccessMut: IndexAccess {
    fn view_mut<I: Into<IndexAddress>>(&self, address: I) -> ViewMut;

    fn index_mut<'a, I, T>(&'a self, address: I) -> T
        where
            I: Into<IndexAddress>,
            T: FromView<ViewMut<'a>>,
    {
        T::from_view(self.view_mut(address))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct IndexAddress {
    name: String,
    bytes: Option<Vec<u8>>,
}

impl IndexAddress {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_ref().map(Vec::as_slice)
    }

    pub fn keyed(&self, key: &[u8]) -> (&str, Vec<u8>) {
        (&self.name, {
            let mut bytes = self.bytes.clone().unwrap_or(vec![]);
            bytes.extend(key);
            bytes
        })
    }
}

impl<'a> From<&'a str> for IndexAddress {
    fn from(name: &'a str) -> Self {
        IndexAddress {
            name: name.to_owned(),
            bytes: None,
        }
    }
}

impl<'a, K: BinaryKey + ?Sized> From<(&'a str, &'a K)> for IndexAddress {
    fn from((name, key): (&'a str, &'a K)) -> Self {
        IndexAddress {
            name: name.to_owned(),
            bytes: Some(key_bytes(key)),
        }
    }
}

impl IndexAccess for Box<dyn Snapshot> {
    fn view<I: Into<IndexAddress>>(&self, address: I) -> View {
        View {
            snapshot: self.as_ref(),
            address: address.into(),
            changes: None,
        }
    }
}

impl<'a, T: IndexAccess> IndexAccess for &'a T {
    fn view<I: Into<IndexAddress>>(&self, address: I) -> View {
        (**self).view(address)
    }
}

impl<'a, T: IndexAccessMut> IndexAccessMut for &'a T {
    fn view_mut<I: Into<IndexAddress>>(&self, address: I) -> ViewMut {
        (**self).view_mut(address)
    }
}

pub trait FromView<T: ReadView> {
    fn from_view(view: T) -> Self;
}

fn key_bytes<K: BinaryKey + ?Sized>(key: &K) -> Vec<u8> {
    let mut buffer = vec![0u8; key.size()];
    key.write(&mut buffer);
    buffer
}

/// TODO
pub trait ReadView {
    /// Returns a value corresponding to the specified key as a raw vector of bytes,
    /// or `None` if it does not exist.
    fn get_bytes(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// Returns `true` if the snapshot contains a value for the specified key.
    ///
    /// Default implementation checks existence of the value using [`get`](#tymethod.get).
    fn contains_raw_key(&self, key: &[u8]) -> bool {
        self.get_bytes(key).is_some()
    }

    /// Returns an iterator over the entries of the snapshot in ascending order starting from
    /// the specified key. The iterator element type is `(&[u8], &[u8])`.
    fn iter_bytes(&self, from: &[u8]) -> BytesIter;

    /// Returns a value of *any* type corresponding to the key of *any* type.
    fn get<K, V>(&self, key: &K) -> Option<V>
        where
            K: BinaryKey + ?Sized,
            V: BinaryValue,
    {
        //TODO: remove unwrap
        self.get_bytes(&key_bytes(key))
            .map(|v| BinaryValue::from_bytes(Cow::Owned(v)).unwrap())
    }

    /// Returns `true` if the index contains a value of *any* type for the specified key of
    /// *any* type.
    fn contains<K>(&self, key: &K) -> bool
        where
            K: BinaryKey + ?Sized,
    {
        self.contains_raw_key(&key_bytes(key))
    }

    /// Returns an iterator over the entries of the index in ascending order. The iterator element
    /// type is *any* key-value pair. An argument `subprefix` allows specifying a subset of keys
    /// for iteration.
    fn iter<P, K, V>(&self, subprefix: &P) -> Iter<K, V>
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
    fn iter_from<P, F, K, V>(&self, subprefix: &P, from: &F) -> Iter<K, V>
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
}

impl<'a> ReadView for View<'a> {
    fn get_bytes(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(ref changes) = self.changes {
            if let Some(change) = changes.data.get(key) {
                match *change {
                    Change::Put(ref v) => return Some(v.clone()),
                    Change::Delete => return None,
                }
            }

            if changes.is_cleared() {
                return None;
            }
        }

        let (name, key) = self.address.keyed(key);
        self.snapshot.get(name, &key)
    }

    fn contains_raw_key(&self, key: &[u8]) -> bool {
        if let Some(ref changes) = self.changes {
            if let Some(change) = changes.data.get(key) {
                match *change {
                    Change::Put(..) => return true,
                    Change::Delete => return false,
                }
            }

            if changes.is_cleared() {
                return false;
            }
        }

        let (name, key) = self.address.keyed(key);
        self.snapshot.contains(name, &key)
    }

    fn iter_bytes(&self, from: &[u8]) -> BytesIter {
        use std::collections::Bound::*;

        let (name, key) = self.address.keyed(from);
        let prefix = self.address.bytes.clone().unwrap_or(vec![]);

        let changes_iter = self.changes
            .as_ref()
            .map(|changes| changes.data.range::<[u8], _>((Included(from), Unbounded)));

        if self.changes
            .as_ref()
            .map_or(false, |changes| changes.is_cleared())
            {
                // Ignore all changes from the snapshot
                Box::new(ChangesIter::new(changes_iter.unwrap()))
            } else {
            Box::new(ForkIter::new(
                Box::new(SnapshotIter::new(self.snapshot, name, prefix, &key)),
                changes_iter,
            ))
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

impl<'a> ReadView for ViewMut<'a> {
    fn get_bytes(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(change) = self.changes.data.get(key) {
            match *change {
                Change::Put(ref v) => return Some(v.clone()),
                Change::Delete => return None,
            }
        }

        if self.changes.is_cleared() {
            return None;
        }

        let (name, key) = self.address.keyed(key);
        self.snapshot.get(name, &key)
    }

    fn contains_raw_key(&self, key: &[u8]) -> bool {
        if let Some(change) = self.changes.data.get(key) {
            match *change {
                Change::Put(..) => return true,
                Change::Delete => return false,
            }
        }

        if self.changes.is_cleared() {
            return false;
        }

        let (name, key) = self.address.keyed(key);
        self.snapshot.contains(name, &key)
    }

    fn iter_bytes(&self, from: &[u8]) -> BytesIter {
        use std::collections::Bound::*;

        let (name, key) = self.address.keyed(from);
        let prefix = self.address.bytes.clone().unwrap_or(vec![]);

        let changes_iter = self.changes
            .data
            .range::<[u8], _>((Included(from), Unbounded));

        if self.changes.is_cleared() {
            // Ignore all changes from the snapshot
            Box::new(ChangesIter::new(changes_iter))
        } else {
            Box::new(ForkIter::new(
                Box::new(SnapshotIter::new(self.snapshot, name, prefix, &key)),
                Some(changes_iter),
            ))
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

impl<'a> ViewMut<'a> {
    pub(super) fn new(
        snapshot: &'a dyn Snapshot,
        address: IndexAddress,
        changes: RefMut<'a, ViewChanges>,
    ) -> Self {
        ViewMut {
            snapshot,
            address,
            changes,
        }
    }

    /// Inserts a key-value pair into the fork.
    pub fn put<K, V>(&mut self, key: &K, value: V)
        where
            K: BinaryKey + ?Sized,
            V: BinaryValue,
    {
        let key = key_bytes(key);
        self.changes
            .data
            .insert(key, Change::Put(value.into_bytes()));
    }

    /// Removes a key from the view.
    pub fn remove<K>(&mut self, key: &K)
        where
            K: BinaryKey + ?Sized,
    {
        self.changes.data.insert(key_bytes(key), Change::Delete);
    }

    /// Clears the view removing all its elements.
    pub fn clear(&mut self) {
        self.changes.clear();
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
                return Some((K::read(k), V::from_bytes(Cow::Borrowed(v)).unwrap()));
            }
        }

        self.ended = true;
        None
    }
}
