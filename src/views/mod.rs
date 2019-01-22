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

use std::{borrow::Cow, fmt, iter::Peekable, marker::PhantomData};

use super::{
    db::{Change, ChangesRef, ForkIter, ViewChanges}, Fork, Iter as BytesIter,
    Iterator as BytesIterator, Snapshot, BinaryKey, BinaryValue,
};

#[cfg(test)]
mod tests;

/// TODO
pub struct View<T: IndexAccess> {
    pub address: IndexAddress,
    pub snapshot: T,
    pub changes: T::Changes,
}

impl<T: IndexAccess> fmt::Debug for View<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("View")
            .field("address",&self.address)
            .finish()
    }
}

pub trait ChangeSet {
    fn as_ref(&self) -> Option<&ViewChanges>;
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

impl<'a> ChangeSet for ChangesRef<'a> {
    fn as_ref(&self) -> Option<&ViewChanges> {
        Some(&*self)
    }
    fn as_mut(&mut self) -> Option<&mut ViewChanges> {
        Some(&mut *self)
    }
}

pub trait IndexAccess: Clone {
    type Changes: ChangeSet;

    fn snapshot(&self) -> &dyn Snapshot;
    fn changes(&self, address: &IndexAddress) -> Self::Changes;
}

pub struct Mount<T> {
    view: T,
}

impl <T: IndexAccess> Mount<T> {
    pub fn new(view: T) -> Self {
        Self {
            view
        }
    }

    pub fn mount<S: AsRef<str>>(self, index_name: S) -> View<T> {
        let address = IndexAddress::root().append_name(index_name.as_ref());
        View {
            snapshot: self.view.clone(),
            changes: self.view.changes(&address),
            address,
        }
    }

    pub fn mount2<S: AsRef<str>, I>(self, index_name: S, index_id: &I) -> View<T>
    where
        I:BinaryKey + ?Sized,
    {
        let address = IndexAddress::root()
            .append_name(index_name.as_ref()).append_bytes(index_id);
        View {
            snapshot: self.view.clone(),
            changes: self.view.changes(&address),
            address,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct IndexAddress {
    name: String,
    bytes: Option<Vec<u8>>,
}

impl IndexAddress {
    pub fn root() -> Self {
        Self {
            name: "".to_owned(),
            bytes: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_ref().map(Vec::as_slice)
    }

    pub fn keyed<'a>(&self, key: &'a [u8]) -> (&str, Cow<'a, [u8]>) {
        (&self.name, match self.bytes {
            None => Cow::Borrowed(key),
            Some(ref bytes) => {
                let mut bytes = bytes.clone();
                bytes.extend(key);
                bytes.into()
            }
        })
    }

    pub fn append_name(&self, suffix: &str) -> Self {
        Self {
            name: if self.name.is_empty() {
                suffix.to_owned()
            } else {
                format!("{}.{}", self.name, suffix)
            },

            bytes: self.bytes.clone(),
        }
    }

    pub fn append_bytes<K: BinaryKey + ?Sized>(&self, suffix: &K) -> Self {
        let suffix = key_bytes(suffix);
        let (name, bytes) = self.keyed(&suffix);

        Self {
            name: name.to_owned(),
            bytes: Some(bytes.into_owned()),
        }
    }
}

// TODO: remove
impl<'a> From<&'a str> for IndexAddress {
    fn from(name: &'a str) -> Self {
        IndexAddress {
            name: name.to_owned(),
            bytes: None,
        }
    }
}

// TODO: remove
impl<'a, K: BinaryKey + ?Sized> From<(&'a str, &'a K)> for IndexAddress {
    fn from((name, key): (&'a str, &'a K)) -> Self {
        IndexAddress {
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

fn key_bytes<K: BinaryKey + ?Sized>(key: &K) -> Vec<u8> {
    let mut buffer = vec![0_u8; key.size()];
    key.write(&mut buffer);
    buffer
}

impl<T: IndexAccess> View<T> {
    fn get_bytes(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(ref changes) = self.changes.as_ref() {
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
        self.snapshot.snapshot().get(name, &key)
    }

    fn contains_raw_key(&self, key: &[u8]) -> bool {
        if let Some(ref changes) = self.changes.as_ref() {
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
        self.snapshot.snapshot().contains(name, &key)
    }

    fn iter_bytes(&self, from: &[u8]) -> BytesIter {
        use std::collections::Bound::*;

        let (name, key) = self.address.keyed(from);
        let prefix = self.address.bytes.clone().unwrap_or_else(|| vec![]);

        let changes_iter = self.changes
            .as_ref()
            .map(|changes| changes.data.range::<[u8], _>((Included(from), Unbounded)));

        let is_cleared = self.changes
            .as_ref()
            .map_or(false, |changes| changes.is_cleared());

        if is_cleared {
            // Ignore all changes from the snapshot
            Box::new(ChangesIter::new(changes_iter.unwrap()))
        } else {
            Box::new(ForkIter::new(
                Box::new(SnapshotIter::new(
                    self.snapshot.snapshot(),
                    name,
                    prefix,
                    &key,
                )),
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
        //TODO: revert
        self.get_bytes(&key_bytes(key))
            .map(|v| BinaryValue::from_bytes(Cow::Owned(v)).unwrap())
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

impl<'a> View<&'a Fork> {
    fn changes(&mut self) -> &mut ViewChanges {
        self.changes.as_mut().unwrap()
    }

    /// Inserts a key-value pair into the fork.
    pub fn put<K, V>(&mut self, key: &K, value: V)
        where
            K: BinaryKey + ?Sized,
            V: BinaryValue,
    {
        let key = key_bytes(key);
        self.changes()
            .data
            .insert(key, Change::Put(value.into_bytes()));
    }

    /// Removes a key from the view.
    pub fn remove<K>(&mut self, key: &K)
        where
            K: BinaryKey + ?Sized,
    {
        self.changes().data.insert(key_bytes(key), Change::Delete);
    }

    /// Clears the view removing all its elements.
    pub fn clear(&mut self) {
        self.changes().clear();
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
                //TODO: revert
                return Some((K::read(k), V::from_bytes(Cow::Borrowed(v)).unwrap()));
            }
        }

        self.ended = true;
        None
    }
}
