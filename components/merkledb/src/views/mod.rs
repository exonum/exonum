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

pub use self::{
    address::{IndexAddress, ResolvedAddress},
    metadata::{get_object_hash, BinaryAttribute, IndexState, IndexType, ViewWithMetadata},
    system_info::SystemInfo,
};

use std::{borrow::Cow, fmt, iter::Peekable, marker::PhantomData};

use self::address::key_bytes;
use super::{
    db::{Change, ChangesMut, ChangesRef, ForkIter, ViewChanges},
    BinaryKey, BinaryValue, Iter as BytesIter, Iterator as BytesIterator, Snapshot,
};

mod address;
mod metadata;
mod system_info;
#[cfg(test)]
mod tests;

/// Represents current view of the database by specified `address` and
/// changes that took place after that view had been created. `View`
/// implementation provides an interface to work with related `changes`.
pub struct View<T: RawAccess> {
    address: ResolvedAddress,
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

/// Allows to read data from the database.
///
/// This trait is rarely needs to be used directly; [`Access`] is a more high-level trait
/// encompassing access to database.
///
/// [`Access`]: trait.Access.html
pub trait RawAccess: Clone {
    /// Type of the `changes()` that will be applied to the database.
    type Changes: ChangeSet;

    /// Reference to a `Snapshot`.
    fn snapshot(&self) -> &dyn Snapshot;
    /// Returns changes related to specific `address` compared to the `snapshot()`.
    fn changes(&self, address: &ResolvedAddress) -> Self::Changes;
}

/// Allows to mutate data in indexes.
///
/// This is a marker trait that is used as a bound for mutable operations on indexes.
/// It can be used in the same way for high-level database objects:
///
/// ```
/// use exonum_merkledb::{access::{Access, RawAccessMut}, ListIndex, MapIndex};
///
/// pub struct Schema<T: Access> {
///     list: ListIndex<T::Base, String>,
///     map: MapIndex<T::Base, u64, u64>,
/// }
///
/// impl<T: Access> Schema<T>
/// where
///     T::Base: RawAccessMut,
/// {
///     pub fn mutate(&mut self) {
///         self.list.push("foo".to_owned());
///         self.map.put(&1, 2);
///     }
/// }
/// ```
pub trait RawAccessMut: RawAccess {}

impl<'a, T> RawAccessMut for T where T: RawAccess<Changes = ChangesMut<'a>> {}

/// Converts index access to a readonly presentation. The conversion operation is cheap.
pub trait AsReadonly: RawAccess {
    /// Readonly version of the access.
    type Readonly: RawAccess;

    /// Performs the conversion.
    fn as_readonly(&self) -> Self::Readonly;
}

macro_rules! impl_snapshot_access {
    ($typ:ty) => {
        impl RawAccess for $typ {
            type Changes = ();

            fn snapshot(&self) -> &dyn Snapshot {
                self.as_ref()
            }

            fn changes(&self, _address: &ResolvedAddress) -> Self::Changes {}
        }

        impl AsReadonly for $typ {
            type Readonly = Self;

            fn as_readonly(&self) -> Self::Readonly {
                self.clone()
            }
        }
    };
}

impl_snapshot_access!(&'_ dyn Snapshot);
impl_snapshot_access!(&'_ Box<dyn Snapshot>);
impl_snapshot_access!(std::rc::Rc<dyn Snapshot>);
impl_snapshot_access!(std::sync::Arc<dyn Snapshot>);

impl<T: RawAccess> View<T> {
    /// Creates a new view for an index with the specified address.
    pub(crate) fn new(index_access: T, address: impl Into<ResolvedAddress>) -> Self {
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

            if changes.is_cleared() {
                return None;
            }
        }

        self.snapshot().get(&self.address, key)
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

        self.snapshot().contains(&self.address, &key)
    }

    fn iter_bytes(&self, from: &[u8]) -> BytesIter<'_> {
        use std::collections::Bound::*;

        let changes_iter = self
            .changes
            .as_ref()
            .map(|changes| changes.data.range::<[u8], _>((Included(from), Unbounded)));

        let is_cleared = self.changes.as_ref().map_or(false, ViewChanges::is_cleared);
        if is_cleared {
            // Ignore all changes from the snapshot.
            Box::new(ChangesIter::new(changes_iter.unwrap()))
        } else {
            Box::new(ForkIter::new(
                self.snapshot().iter(&self.address, from),
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

    /// Sets a key / value pair in the view storage, unless the view is backed by a readonly access
    /// (in which case, the changes are forgotten).
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

    /// Sets the aggregation flag for the view, unless the view is backed by a readonly access
    /// (in which case, the flag is forgotten).
    ///
    /// The aggregation flag is used by `Fork::into_patch()` to update the state aggregator.
    pub(crate) fn set_or_forget_aggregation(&mut self, is_aggregated: bool) {
        if let Some(changes) = self.changes.as_mut() {
            changes.set_aggregation(is_aggregated);
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
