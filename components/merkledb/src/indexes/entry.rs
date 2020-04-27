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

//! An implementation of index that may only contain one element.

use std::marker::PhantomData;

use crate::{
    access::{Access, AccessError, FromAccess},
    views::{IndexAddress, IndexType, RawAccess, RawAccessMut, View, ViewWithMetadata},
    BinaryValue,
};

/// An index that may only contain one element.
///
/// You can add an element to this index and check whether it exists. A value
/// should implement [`BinaryValue`] trait.
///
/// [`BinaryValue`]: ../trait.BinaryValue.html
#[derive(Debug)]
pub struct Entry<T: RawAccess, V> {
    base: View<T>,
    _v: PhantomData<V>,
}

impl<T, V> FromAccess<T> for Entry<T::Base, V>
where
    T: Access,
    V: BinaryValue,
{
    fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError> {
        let view = access.get_or_create_view(addr, IndexType::Entry)?;
        Ok(Self::new(view))
    }
}

impl<T, V> Entry<T, V>
where
    T: RawAccess,
    V: BinaryValue,
{
    fn new(view: ViewWithMetadata<T>) -> Self {
        let base = view.into();
        Self {
            base,
            _v: PhantomData,
        }
    }

    /// Returns a value of the entry or `None` if does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_entry("name");
    /// assert_eq!(None, index.get());
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    /// ```
    pub fn get(&self) -> Option<V> {
        self.base.get(&())
    }

    /// Returns `true` if a value of the entry exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_entry("name");
    /// assert!(!index.exists());
    ///
    /// index.set(10);
    /// assert!(index.exists());
    /// ```
    pub fn exists(&self) -> bool {
        self.base.contains(&())
    }
}

impl<T, V> Entry<T, V>
where
    T: RawAccessMut,
    V: BinaryValue,
{
    /// Changes a value of the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_entry("name");
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    /// ```
    pub fn set(&mut self, value: V) {
        self.base.put(&(), value)
    }

    /// Removes a value of the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_entry("name");
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    ///
    /// index.remove();
    /// assert_eq!(None, index.get());
    /// ```
    pub fn remove(&mut self) {
        self.base.remove(&())
    }

    /// Takes the value out of the entry, leaving a None in its place.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_entry("name");
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    ///
    /// let value = index.take();
    /// assert_eq!(Some(10), value);
    /// assert_eq!(None, index.get());
    /// ```
    pub fn take(&mut self) -> Option<V> {
        let value = self.get();
        if value.is_some() {
            self.remove();
        }
        value
    }

    /// Replaces the value in the entry with the given one, returning the previously stored value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    /// let mut index = fork.get_entry("name");
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    ///
    /// let value = index.swap(20);
    /// assert_eq!(Some(10), value);
    /// assert_eq!(Some(20), index.get());
    /// ```
    pub fn swap(&mut self, value: V) -> Option<V> {
        let previous = self.get();
        self.set(value);
        previous
    }
}
