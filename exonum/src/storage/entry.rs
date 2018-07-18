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

//! An implementation of index that may only contain one element.

use std::marker::PhantomData;

use super::{
    base_index::{BaseIndex, BaseIndexForked, BaseIndexMut},
    indexes_metadata::IndexType, Fork, DbView, DbViewMut, Result,
    StorageValue
};
use crypto::Hash;

/// An index that may only contain one element.
///
/// A value should implement [`StorageValue`] trait.
///
/// [`StorageValue`]: trait.StorageValue.html
#[derive(Debug)]
pub struct Entry<T, V> {
    base: BaseIndex<T>,
    _v: PhantomData<V>,
}

pub trait EntryMut<V>
where
    V: StorageValue,
{
    fn set(&mut self, value: V) -> Result<()>;

    fn remove(&mut self) -> Result<()>;
}

pub trait EntryForked<V>
where
    V: StorageValue,
{
    fn set(&mut self, value: V);

    fn remove(&mut self);
}

impl<T, V> Entry<T, V>
where
    T: AsRef<DbView>,
    V: StorageValue,
{
    /// Creates a new index representation based on the name and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: trait.Snapshot.html
    /// [`&mut Fork`]: struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, Entry};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: Entry<_, u8> = Entry::new(name, &snapshot);
    /// ```
    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {
        Entry {
            base: BaseIndex::new(index_name.as_ref(), IndexType::Entry, view),
            _v: PhantomData,
        }
    }

    /// Returns a value of the entry or `None` if does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, Entry};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = Entry::new(name, &mut fork);
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
    /// use exonum::storage::{MemoryDB, Database, Entry};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = Entry::new(name, &mut fork);
    /// assert!(!index.exists());
    ///
    /// index.set(10);
    /// assert!(index.exists());
    /// ```
    pub fn exists(&self) -> bool {
        self.base.contains(&())
    }

    /// Returns hash of the entry or default hash value if does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, Entry};
    /// use exonum::crypto::{self, Hash};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = Entry::new(name, &mut fork);
    /// assert_eq!(Hash::default(), index.hash());
    ///
    /// let value = 10;
    /// index.set(value);
    /// assert_eq!(crypto::hash(&[value]), index.hash());
    /// ```
    pub fn hash(&self) -> Hash {
        self.base
            .get::<(), V>(&())
            .map(|v| v.hash())
            .unwrap_or_default()
    }
}

impl<T, V> EntryMut<V> for Entry<T, V>
where
    T: AsRef<DbView>,
    T: AsMut<DbViewMut>,
    V: StorageValue,
{
    fn set(&mut self, value: V) -> Result<()> {
        self.base.put(&(), value)
    }

    fn remove(&mut self) -> Result<()> {
        self.base.remove(&())
    }
}

impl<'a, V> EntryForked<V> for Entry<&'a mut Fork, V>
where
    V: StorageValue,
{
    /// Changes a value of the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, Entry};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = Entry::new(name, &mut fork);
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    /// ```
    fn set(&mut self, value: V) {
        self.base.put(&(), value);
    }

    /// Removes a value of the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, Entry};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = Entry::new(name, &mut fork);
    ///
    /// index.set(10);
    /// assert_eq!(Some(10), index.get());
    ///
    /// index.remove();
    /// assert_eq!(None, index.get());
    /// ```
    fn remove(&mut self) {
        self.base.remove(&());
    }
}
