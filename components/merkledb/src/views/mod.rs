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

pub use self::metadata::{BinaryAttribute, IndexState, IndexType, ViewWithMetadata};

use std::{borrow::Cow, fmt, iter::Peekable, marker::PhantomData};

use super::{
    db::{Change, ChangesMut, ChangesRef, ForkIter, ViewChanges},
    BinaryKey, BinaryValue, Iter as BytesIter, Iterator as BytesIterator, Snapshot,
};

mod metadata;
#[cfg(test)]
mod tests;

/// Separator between the name and the additional bytes in family indexes.
const INDEX_NAME_SEPARATOR: &[u8] = &[0];

/// Represents current view of the database by specified `address` and
/// changes that took place after that view had been created. `View`
/// implementation provides an interface to work with related `changes`.
pub struct View<T: RawAccess> {
    address: IndexAddress,
    index_access: T,
    changes: T::Changes,
}

impl<T: RawAccess> fmt::Debug for View<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("View")
            .field("address", &self.address)
            .finish()
    }
}

/// Utility trait to provide optional references to `ViewChanges`.
pub trait ChangeSet {
    fn as_ref(&self) -> Option<&ViewChanges>;
    /// Provides mutable reference to changes. The implementation for a `RawAccessMut` type
    /// should always return `Some(_)`.
    fn as_mut(&mut self) -> Option<&mut ViewChanges>;
}

/// No-op implementation used in `Snapshot`.
impl ChangeSet for () {
    fn as_ref(&self) -> Option<&ViewChanges> {
        None
    }
    fn as_mut(&mut self) -> Option<&mut ViewChanges> {
        None
    }
}

impl ChangeSet for ChangesRef {
    fn as_ref(&self) -> Option<&ViewChanges> {
        Some(&*self)
    }
    fn as_mut(&mut self) -> Option<&mut ViewChanges> {
        None
    }
}

impl ChangeSet for ChangesMut<'_> {
    fn as_ref(&self) -> Option<&ViewChanges> {
        Some(&*self)
    }
    fn as_mut(&mut self) -> Option<&mut ViewChanges> {
        Some(&mut *self)
    }
}

/// Allows to read data from indexes.
pub trait RawAccess: Clone {
    /// Type of the `changes` that will be applied to the database.
    ///
    /// In case of `snapshot` changes are represented by the empty type `()`,
    /// because `snapshot` is read-only.
    type Changes: ChangeSet;

    /// Reference to `Snapshot` used in `View` implementation.
    fn snapshot(&self) -> &dyn Snapshot;
    /// Returns changes related to specific `address` compared to the `snapshot()`.
    fn changes(&self, address: &IndexAddress) -> Self::Changes;
}

/// Allows to mutate data in indexes.
pub trait RawAccessMut: RawAccess {}

impl<'a, T> RawAccessMut for T where T: RawAccess<Changes = ChangesMut<'a>> {}

/// Converts index access to a readonly presentation.
pub trait ToReadonly: RawAccess {
    /// Readonly version of the access.
    type Readonly: RawAccess;

    /// Performs the conversion.
    fn to_readonly(&self) -> Self::Readonly;
}

/// Represents address of the index in the database.
///
/// # Examples
///
/// `IndexAddress` can be used implicitly, since `&str` and `(&str, &impl BinaryKey)` can both
/// be converted into an address.
///
/// ```
/// use exonum_merkledb::{access::AccessExt, IndexAddress, TemporaryDB, Database};
///
/// let db = TemporaryDB::new();
/// let fork = db.fork();
///
/// // Using a string address:
/// let map = fork.as_ref().get_map::<_, String, u8>("map");
/// // Using an address within an index family:
/// let list = fork.as_ref().get_list::<_, String>(("index", &3_u32));
/// // Using `IndexAddress` explicitly:
/// let addr = IndexAddress::with_root("data").append_bytes(&vec![1, 2, 3]);
/// let set = fork.as_ref().get_value_set::<_, u64>(addr);
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct IndexAddress {
    pub(super) name: String,
    pub(super) bytes: Option<Vec<u8>>,
}

impl IndexAddress {
    /// Creates empty `IndexAddress`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates new `IndexAddress` with specified `root` name.
    pub fn with_root<S: Into<String>>(root: S) -> Self {
        Self {
            name: root.into(),
            bytes: None,
        }
    }

    /// Returns name part of `IndexAddress`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns bytes part of `IndexAddress`.
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_ref().map(Vec::as_slice)
    }

    /// Returns tuple consists of `name` and `bytes` concatenated with provided `key`.
    /// This is used to obtain single value(serialized as byte array) from the database.
    pub(crate) fn keyed<'a>(&self, key: &'a [u8]) -> (&str, Cow<'a, [u8]>) {
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

    /// Prepends a name part to `IndexAddress`.
    pub fn prepend_name<'a>(self, prefix: impl Into<Cow<'a, str>>) -> Self {
        let prefix = prefix.into();
        Self {
            name: if self.name.is_empty() {
                prefix.into_owned()
            } else {
                // Because `concat` is faster than `format!("...")` in all cases.
                [prefix.as_ref(), ".", self.name()].concat()
            },

            bytes: self.bytes,
        }
    }

    /// Appends a bytes part to `IndexAddress`.
    pub fn append_bytes<K: BinaryKey + ?Sized>(self, suffix: &K) -> Self {
        let name = self.name;
        let bytes = if let Some(ref bytes) = self.bytes {
            concat_keys!(bytes, suffix)
        } else {
            concat_keys!(suffix)
        };

        Self {
            name,
            bytes: Some(bytes),
        }
    }

    /// Full address with a separator between `name` and `bytes` represented as byte array.
    pub fn fully_qualified_name(&self) -> Vec<u8> {
        if let Some(bytes) = self.bytes() {
            concat_keys!(self.name(), INDEX_NAME_SEPARATOR, bytes)
        } else {
            concat_keys!(self.name())
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

// TODO should we have this impl in public interface? ECR-2834
impl<'a, K: BinaryKey + ?Sized> From<(&'a str, &'a K)> for IndexAddress {
    fn from((name, key): (&'a str, &'a K)) -> Self {
        Self {
            name: name.to_owned(),
            bytes: Some(key_bytes(key)),
        }
    }
}

macro_rules! impl_snapshot_access {
    ($typ:ty) => {
        impl RawAccess for $typ {
            type Changes = ();

            fn snapshot(&self) -> &dyn Snapshot {
                self.as_ref()
            }

            fn changes(&self, _address: &IndexAddress) -> Self::Changes {}
        }

        impl ToReadonly for $typ {
            type Readonly = Self;

            fn to_readonly(&self) -> Self::Readonly {
                self.clone()
            }
        }
    };
}
impl_snapshot_access!(&'_ dyn Snapshot);
impl_snapshot_access!(&'_ Box<dyn Snapshot>);
impl_snapshot_access!(std::rc::Rc<dyn Snapshot>);
impl_snapshot_access!(std::sync::Arc<dyn Snapshot>);

fn key_bytes<K: BinaryKey + ?Sized>(key: &K) -> Vec<u8> {
    concat_keys!(key)
}

impl<T: RawAccess> View<T> {
    /// Creates a new view for an index with the specified address.
    #[doc(hidden)]
    // ^-- This should be `pub(crate)`, but is currently used by the testkit to revert blocks.
    pub fn new<I: Into<IndexAddress>>(index_access: T, address: I) -> Self {
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

    fn iter_bytes(&self, from: &[u8]) -> BytesIter<'_> {
        use std::collections::Bound::*;

        let (name, key) = self.address.keyed(from);
        let prefix = self.address.bytes.clone().unwrap_or_else(|| vec![]);

        let changes_iter = self
            .changes
            .as_ref()
            .map(|changes| changes.data.range::<[u8], _>((Included(from), Unbounded)));

        let is_empty = self.changes.as_ref().map_or(false, ViewChanges::is_empty);

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
    pub fn iter<P, K, V>(&self, subprefix: &P) -> Iter<'_, K, V>
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
    pub fn iter_from<P, F, K, V>(&self, subprefix: &P, from: &F) -> Iter<'_, K, V>
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

    /// Crutch to be able to create metadata for indexes not present in the storage.
    ///
    /// # Return value
    ///
    /// Returns whether the changes were saved.
    pub(crate) fn put_or_forget<K, V>(&mut self, key: &K, value: V) -> bool
    where
        K: BinaryKey + ?Sized,
        V: BinaryValue,
    {
        if let Some(changes) = self.changes.as_mut() {
            changes
                .data
                .insert(concat_keys!(key), Change::Put(value.into_bytes()));
            true
        } else {
            false
        }
    }
}

impl<T: RawAccessMut> View<T> {
    /// Inserts a key-value pair into the fork.
    pub fn put<K, V>(&mut self, key: &K, value: V)
    where
        K: BinaryKey + ?Sized,
        V: BinaryValue,
    {
        self.changes
            .as_mut()
            .unwrap()
            .data
            .insert(concat_keys!(key), Change::Put(value.into_bytes()));
    }

    /// Removes a key from the view.
    pub fn remove<K>(&mut self, key: &K)
    where
        K: BinaryKey + ?Sized,
    {
        self.changes
            .as_mut()
            .unwrap()
            .data
            .insert(concat_keys!(key), Change::Delete);
    }

    /// Clears the view removing all its elements.
    pub fn clear(&mut self) {
        self.changes.as_mut().unwrap().clear();
    }
}

/// Iterator over entries in a snapshot limited to a specific view.
struct SnapshotIter<'a> {
    inner: BytesIter<'a>,
    prefix: Vec<u8>,
    ended: bool,
}

impl<'a> fmt::Debug for SnapshotIter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl BytesIterator for SnapshotIter<'_> {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
