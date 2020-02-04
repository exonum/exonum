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

pub use self::{
    address::{IndexAddress, ResolvedAddress},
    metadata::{
        get_object_hash, BinaryAttribute, GroupKeys, IndexMetadata, IndexState, IndexType,
        IndexesPool, ViewWithMetadata,
    },
    system_schema::{get_state_aggregator, SystemSchema},
};

use std::{borrow::Cow, fmt, iter::Peekable, marker::PhantomData};

use self::address::key_bytes;
use super::{
    db::{Change, ChangesMut, ChangesRef, ForkIter, ViewChanges},
    BinaryKey, BinaryValue, Iter as BytesIter, Iterator as BytesIterator, Snapshot,
};

mod address;
mod metadata;
mod system_schema;
#[cfg(test)]
mod tests;

/// Represents current view of the database by specified `address` and
/// changes that took place after that view had been created. `View`
/// implementation provides an interface to work with related `changes`.
#[derive(Debug)]
pub(crate) enum View<T: RawAccess> {
    Real(ViewInner<T>),
    Phantom,
}

pub struct ViewInner<T: RawAccess> {
    address: ResolvedAddress,
    index_access: T,
    changes: T::Changes,
}

impl<T: RawAccess> fmt::Debug for ViewInner<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ViewInner")
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

impl ChangeSet for ChangesRef<'_> {
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

/// Allows to read data from the database. The data consists of a snapshot and
/// changes relative to this snapshot. Depending on the implementation, the changes
/// can be empty, immutable or mutable.
///
/// This trait is rarely needs to be used directly; [`Access`] is a more high-level trait
/// encompassing access to database. In particular, using `snapshot()` method to convert
/// the implementation into `&dyn Snapshot` is logically incorrect, because the snapshot
/// may not reflect the most recent state of `RawAccess`.
///
/// [`Access`]: trait.Access.html
pub trait RawAccess: Clone {
    /// Type of the `changes()` that will be applied to the database.
    type Changes: ChangeSet;

    /// Reference to a `Snapshot`. This is the base relative to which the changes are defined.
    fn snapshot(&self) -> &dyn Snapshot;
    /// Returns changes related to specific `address` compared to the `snapshot()`.
    fn changes(&self, address: &ResolvedAddress) -> Self::Changes;
}

/// Allows to mutate data in indexes.
///
/// This is a marker trait that is used as a bound for mutable operations on indexes.
/// It can be used in the same way for high-level database objects:
///
/// # Example
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

impl<T: RawAccess> ViewInner<T> {
    fn snapshot(&self) -> &dyn Snapshot {
        self.index_access.snapshot()
    }

    fn get_bytes(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.changes
            .as_ref()
            .map_or(Err(()), |changes| changes.get(key))
            // At this point, `Err(_)` signifies that we need to retrieve data from the snapshot.
            .unwrap_or_else(|()| self.snapshot().get(&self.address, key))
    }

    fn contains_raw_key(&self, key: &[u8]) -> bool {
        self.changes
            .as_ref()
            .map_or(Err(()), |changes| changes.contains(key))
            // At this point, `Err(_)` signifies that we need to retrieve data from the snapshot.
            .unwrap_or_else(|()| self.snapshot().contains(&self.address, key))
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
}

impl<T: RawAccess> View<T> {
    /// Creates a new view for an index with the specified address.
    pub(crate) fn new(index_access: T, address: impl Into<ResolvedAddress>) -> Self {
        let address = address.into();
        let changes = index_access.changes(&address);
        View::Real(ViewInner {
            index_access,
            changes,
            address,
        })
    }

    /// Creates a new phantom view. The phantom views do not borrow changes and do not retain
    /// resolved address / access.
    pub(crate) fn new_phantom() -> Self {
        View::Phantom
    }

    /// Returns the access this view is attached to. If this view is phantom, returns `None`.
    pub(crate) fn access(&self) -> Option<&T> {
        match self {
            View::Real(ViewInner { index_access, .. }) => Some(index_access),
            View::Phantom => None,
        }
    }

    fn get_bytes(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self {
            View::Real(inner) => inner.get_bytes(key),
            View::Phantom => None,
        }
    }

    fn contains_raw_key(&self, key: &[u8]) -> bool {
        match self {
            View::Real(inner) => inner.contains_raw_key(key),
            View::Phantom => false,
        }
    }

    fn iter_bytes(&self, from: &[u8]) -> BytesIter<'_> {
        match self {
            View::Real(inner) => inner.iter_bytes(from),
            View::Phantom => Box::new(EmptyIterator),
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
        K: BinaryKey + ?Sized,
        V: BinaryValue,
    {
        let iter_prefix = key_bytes(subprefix);
        Iter {
            base_iter: self.iter_bytes(&iter_prefix),
            prefix: iter_prefix,
            detach_prefix: false,
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
        K: BinaryKey + ?Sized,
        V: BinaryValue,
    {
        let iter_prefix = key_bytes(subprefix);
        let iter_from = key_bytes(from);
        Iter {
            base_iter: self.iter_bytes(&iter_from),
            prefix: iter_prefix,
            detach_prefix: false,
            ended: false,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Returns an iterator over the entries of the index in ascending order, optionally
    /// starting from the specified key. Unlike `iter_from`, the provided prefix will be detached
    /// from the beginning of each key slice.
    pub fn iter_detached<P, K, V>(&self, detached_prefix: &P, from: Option<&K>) -> Iter<'_, K, V>
    where
        P: BinaryKey,
        K: BinaryKey + ?Sized,
        V: BinaryValue,
    {
        let iter_prefix = key_bytes(detached_prefix);
        let iter_from = if let Some(from) = from {
            Cow::Owned(concat_keys!(detached_prefix, from))
        } else {
            Cow::Borrowed(&*iter_prefix)
        };
        Iter {
            base_iter: self.iter_bytes(&iter_from),
            prefix: iter_prefix,
            detach_prefix: true,
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
        if let View::Real(inner) = self {
            if let Some(changes) = inner.changes.as_mut() {
                changes
                    .data
                    .insert(concat_keys!(key), Change::Put(value.into_bytes()));
                return true;
            }
        }
        false
    }

    /// Sets the aggregation flag for the view, unless the view is backed by a readonly access
    /// (in which case, the flag is forgotten).
    ///
    /// The aggregation flag is used by `Fork::into_patch()` to update the state aggregator.
    pub(crate) fn set_or_forget_aggregation(&mut self, namespace: Option<String>) {
        if let View::Real(ViewInner { changes, .. }) = self {
            if let Some(changes) = changes.as_mut() {
                changes.set_aggregation(namespace);
            }
        }
    }
}

impl<T: RawAccessMut> View<T> {
    fn changes_mut(&mut self) -> &mut ViewChanges {
        const ACCESS_ERROR: &str =
            "Attempt to modify a readonly view of the database using a generic access. \
             The caller should check the access type before calling any mutable methods";

        match self {
            View::Real(ViewInner { changes, .. }) => changes.as_mut().expect(ACCESS_ERROR),
            View::Phantom => panic!(ACCESS_ERROR),
        }
    }

    /// Inserts a key-value pair into the fork.
    pub fn put<K, V>(&mut self, key: &K, value: V)
    where
        K: BinaryKey + ?Sized,
        V: BinaryValue,
    {
        self.changes_mut()
            .data
            .insert(concat_keys!(key), Change::Put(value.into_bytes()));
    }

    /// Removes a key from the view.
    pub fn remove<K>(&mut self, key: &K)
    where
        K: BinaryKey + ?Sized,
    {
        self.changes_mut()
            .data
            .insert(concat_keys!(key), Change::Delete);
    }

    /// Clears the view removing all its elements.
    pub fn clear(&mut self) {
        self.changes_mut().clear();
    }
}

/// A bytes iterator implementation that has no items.
struct EmptyIterator;

impl BytesIterator for EmptyIterator {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        None
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        None
    }
}

pub(crate) struct ChangesIter<'a, T: Iterator + 'a> {
    inner: Peekable<T>,
    _lifetime: PhantomData<&'a ()>,
}

/// Iterator over a set of changes.
impl<'a, T> ChangesIter<'a, T>
where
    T: Iterator<Item = (&'a Vec<u8>, &'a Change)>,
{
    pub fn new(iterator: T) -> Self {
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
                Some((key, Change::Put(ref value))) => {
                    return Some((key.as_slice(), value.as_slice()));
                }
                Some((_, Change::Delete)) => {
                    // Advance the iterator. Since we've already peeked the value,
                    // we can safely drop it.
                    self.inner.next();
                }
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
pub struct Iter<'a, K: ?Sized, V> {
    base_iter: BytesIter<'a>,
    prefix: Vec<u8>,
    detach_prefix: bool,
    ended: bool,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<'a, K: ?Sized, V> fmt::Debug for Iter<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Iter(..)")
    }
}

impl<'a, K, V> Iter<'a, K, V>
where
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    /// Drops the keys returned by the underlying iterator without parsing them.
    pub(crate) fn drop_key_type(self) -> Iter<'a, (), V> {
        Iter {
            base_iter: self.base_iter,
            prefix: self.prefix,
            detach_prefix: self.detach_prefix,
            ended: self.ended,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Drops the values returned by the underlying iterator without parsing them.
    pub(crate) fn drop_value_type(self) -> Iter<'a, K, ()> {
        Iter {
            base_iter: self.base_iter,
            prefix: self.prefix,
            detach_prefix: self.detach_prefix,
            ended: self.ended,
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    type Item = (K::Owned, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }

        if let Some((key_slice, value_slice)) = self.base_iter.next() {
            if key_slice.starts_with(&self.prefix) {
                let key = if self.detach_prefix {
                    // Since we've checked `start_with`, slicing here cannot panic.
                    K::read(&key_slice[self.prefix.len()..])
                } else {
                    K::read(key_slice)
                };
                let value = V::from_bytes(Cow::Borrowed(value_slice))
                    .expect("Unable to decode value from bytes");
                return Some((key, value));
            }
        }

        self.ended = true;
        None
    }
}
