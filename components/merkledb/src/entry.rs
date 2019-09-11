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

//! An implementation of index that may only contain one element.

use std::marker::PhantomData;

use exonum_crypto::Hash;

use crate::views::IndexAddress;
use crate::{
    views::{AnyObject, IndexAccess, IndexBuilder, IndexState, IndexType, View},
    BinaryKey, BinaryValue, ObjectHash,
};

/// An index that may only contain one element.
///
/// You can add an element to this index and check whether it exists. A value
/// should implement [`BinaryValue`] trait.
///
/// [`BinaryValue`]: trait.BinaryValue.html
#[derive(Debug)]
pub struct Entry<T: IndexAccess, V> {
    base: View<T>,
    state: IndexState<T, ()>,
    _v: PhantomData<V>,
}

impl<T, V> AnyObject<T> for Entry<T, V>
where
    T: IndexAccess,
    V: BinaryValue,
{
    fn view(self) -> View<T> {
        self.base
    }

    fn object_type(&self) -> IndexType {
        IndexType::Entry
    }

    fn metadata(&self) -> Vec<u8> {
        self.state.metadata().to_bytes()
    }
}

impl<T, V> Entry<T, V>
where
    T: IndexAccess,
    V: BinaryValue + ObjectHash,
{
    /// Creates a new index representation based on the name and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case, only
    /// immutable methods are available. In the second case, both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: trait.Snapshot.html
    /// [`&mut Fork`]: struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: Entry<_, u8> = Entry::new(name, &snapshot);
    /// ```
    pub fn new<S: Into<String>>(index_name: S, view: T) -> Self {
        let (base, state) = IndexBuilder::new(view)
            .index_type(IndexType::Entry)
            .index_name(index_name)
            .build::<()>();

        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    /// Creates a new index representation based on the name, index ID in family
    /// and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case, only
    /// immutable methods are available. In the second case, both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: trait.Snapshot.html
    /// [`&mut Fork`]: struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let index_id = vec![01];
    /// let snapshot = db.snapshot();
    /// let index: Entry<_, u8> = Entry::new_in_family(name, &index_id, &snapshot);
    /// ```
    pub fn new_in_family<S, I>(family_name: S, index_id: &I, view: T) -> Self
    where
        I: BinaryKey,
        I: ?Sized,
        S: Into<String>,
    {
        let (base, state) = IndexBuilder::new(view)
            .index_type(IndexType::Entry)
            .index_name(family_name)
            .family_id(index_id)
            .build::<()>();

        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    pub(crate) fn get_from<I: Into<IndexAddress>>(address: I, access: T) -> Option<Self> {
        IndexBuilder::from_address(address, access)
            .index_type(IndexType::Entry)
            .build_existed::<()>()
            .map(|(base, state)| Self {
                base,
                state,
                _v: PhantomData,
            })
    }

    pub(crate) fn create_from<I: Into<IndexAddress>>(address: I, access: T) -> Self {
        let (base, state) = IndexBuilder::from_address(address, access)
            .index_type(IndexType::Entry)
            .build::<()>();

        Self {
            base,
            state,
            _v: PhantomData,
        }
    }

    /// Returns a value of the entry or `None` if does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = Entry::new(name, &fork);
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
    /// use exonum_merkledb::{TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = Entry::new(name, &fork);
    /// assert!(!index.exists());
    ///
    /// index.set(10);
    /// assert!(index.exists());
    /// ```
    pub fn exists(&self) -> bool {
        self.base.contains(&())
    }

    /// Changes a value of the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = Entry::new(name, &fork);
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
    /// use exonum_merkledb::{TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = Entry::new(name, &fork);
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
    /// use exonum_merkledb::{TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = Entry::new(name, &fork);
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
    /// use exonum_merkledb::{TemporaryDB, Database, Entry};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = Entry::new(name, &fork);
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

impl<T, V> ObjectHash for Entry<T, V>
where
    T: IndexAccess,
    V: BinaryValue + ObjectHash,
{
    /// Returns hash of the entry or default hash value if does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, Entry, ObjectHash};
    /// use exonum_crypto::{self, Hash};
    ///
    /// let db = TemporaryDB::new();
    /// let name = "name";
    /// let fork = db.fork();
    /// let mut index = Entry::new(name, &fork);
    /// assert_eq!(Hash::default(), index.object_hash());
    ///
    /// let value = 10;
    /// index.set(value);
    /// assert_eq!(exonum_crypto::hash(&[value]), index.object_hash());
    /// ```
    fn object_hash(&self) -> Hash {
        self.base
            .get::<(), V>(&())
            .map(|v| v.object_hash())
            .unwrap_or_default()
    }
}
