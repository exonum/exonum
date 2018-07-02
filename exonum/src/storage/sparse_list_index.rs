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

//! An implementation of array list of items with spaces.

// TODO: Remove when https://github.com/rust-lang-nursery/rust-clippy/issues/2190 is fixed.
#![cfg_attr(feature="cargo-clippy", allow(doc_markdown))]

use byteorder::{BigEndian, ByteOrder};

use std::{borrow::Cow, cell::Cell, marker::PhantomData};

use super::{
    base_index::{BaseIndex, BaseIndexIter}, indexes_metadata::IndexType, Fork, Snapshot,
    StorageKey, StorageValue,
};
use crypto::{hash, CryptoHash, Hash};

#[derive(Debug, Default, Clone, Copy)]
struct SparseListSize {
    /// Total list's length including spaces. In fact points to the next index for a new element.
    capacity: u64,
    /// Amount of non-empty elements.
    length: u64,
}

impl SparseListSize {
    fn to_array(&self) -> [u8; 16] {
        let mut buf = [0; 16];
        BigEndian::write_u64(&mut buf[0..8], self.capacity);
        BigEndian::write_u64(&mut buf[8..16], self.length);
        buf
    }
}

impl CryptoHash for SparseListSize {
    fn hash(&self) -> Hash {
        hash(&self.to_array())
    }
}

impl StorageValue for SparseListSize {
    fn into_bytes(self) -> Vec<u8> {
        self.to_array().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        let buf = value.as_ref();
        let capacity = BigEndian::read_u64(&buf[0..8]);
        let length = BigEndian::read_u64(&buf[8..16]);
        SparseListSize { capacity, length }
    }
}

/// The list of items is similar to the [`ListIndex`], but it may contain "spaces". For instance,
/// list might contain six elements with indexes: "1, 2, 3, 5, 7, 8" (missing 4 and 6). And if you
/// try to get element for index 4 or 6 you'll get None.
///
/// `SparseListIndex` implements an array list, storing the element as values and using `u64`
/// as an index.
/// `SparseListIndex` requires that the elements implement the [`StorageValue`] trait.
///
/// [`StorageValue`]: ../trait.StorageValue.html
/// [`ListIndex`]: <../list_index/struct.ListIndex.html>
#[derive(Debug)]
pub struct SparseListIndex<T, V> {
    base: BaseIndex<T>,
    size: Cell<Option<SparseListSize>>,
    _v: PhantomData<V>,
}

/// An iterator over the items of a `SparseListIndex`.
///
/// This struct is created by the [`iter`] method on [`SparseListIndex`].
/// See its documentation for more.
///
/// [`iter`]: struct.SparseListIndex.html#method.iter
/// [`SparseListIndex`]: struct.SparseListIndex.html
#[derive(Debug)]
pub struct SparseListIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, u64, V>,
}

/// An iterator over the indices of a `SparseListIndex`.
///
/// This struct is created by the [`indices`] method on [`SparseListIndex`].
/// See its documentation for more.
///
/// [`indices`]: struct.SparseListIndex.html#method.indices
/// [`SparseListIndex`]: struct.SparseListIndex.html
#[derive(Debug)]
pub struct SparseListIndexKeys<'a> {
    base_iter: BaseIndexIter<'a, u64, ()>,
}

/// An iterator over the values of a `SparseListIndex`.
///
/// This struct is created by the [`values`] method on [`SparseListIndex`].
/// See its documentation for more.
///
/// [`values`]: struct.SparseListIndex.html#method.values
/// [`SparseListIndex`]: struct.SparseListIndex.html
#[derive(Debug)]
pub struct SparseListIndexValues<'a, V> {
    base_iter: BaseIndexIter<'a, (), V>,
}

impl<T, V> SparseListIndex<T, V>
where
    T: AsRef<Snapshot>,
    V: StorageValue,
{
    /// Creates a new index representation based on the name and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let name = "name";
    /// let index: SparseListIndex<_, u8> = SparseListIndex::new(name, &snapshot);
    /// ```
    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {
        SparseListIndex {
            base: BaseIndex::new(index_name, IndexType::SparseList, view),
            size: Cell::new(None),
            _v: PhantomData,
        }
    }

    /// Creates a new index representation based on the name, index id in family
    /// and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let name = "name";
    /// let index_id = vec![123];
    /// let index: SparseListIndex<_, u8> = SparseListIndex::new_in_family(
    ///     name,
    ///     &index_id,
    ///     &snapshot,
    ///  );
    /// ```
    pub fn new_in_family<S: AsRef<str>, I: StorageKey>(
        family_name: S,
        index_id: &I,
        view: T,
    ) -> Self {
        SparseListIndex {
            base: BaseIndex::new_in_family(family_name, index_id, IndexType::SparseList, view),
            size: Cell::new(None),
            _v: PhantomData,
        }
    }

    fn size(&self) -> SparseListSize {
        if let Some(size) = self.size.get() {
            return size;
        }
        let size = self.base.get(&()).unwrap_or_default();
        self.size.set(Some(size));
        size
    }

    /// Returns an element at that position or `None` if out of bounds or it does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    /// assert_eq!(None, index.get(0));
    ///
    /// index.push(42);
    /// assert_eq!(Some(42), index.get(0));
    /// index.push(1);
    /// index.remove(0);
    /// assert_eq!(None, index.get(0));
    /// assert_eq!(Some(1), index.get(1));
    /// ```
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&index)
    }

    /// Returns `true` if the list contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    /// assert!(index.is_empty());
    ///
    /// index.push(42);
    /// assert!(!index.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the total amount of elements (including "empty" elements) in the list. The value of
    /// capacity is determined by the maximum index of the element ever inserted into the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    /// assert_eq!(0, index.capacity());
    ///
    /// index.push(10);
    /// index.push(12);
    /// assert_eq!(2, index.capacity());
    ///
    /// index.remove(0);
    ///
    /// index.push(100);
    /// assert_eq!(3, index.capacity());
    /// ```
    pub fn capacity(&self) -> u64 {
        self.size().capacity
    }

    /// Returns the total amount of non-empty elements in the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    /// assert_eq!(0, index.len());
    ///
    /// index.push(10);
    /// assert_eq!(1, index.len());
    ///
    /// index.remove(0);
    ///
    /// index.push(100);
    /// assert_eq!(1, index.len());
    /// ```
    pub fn len(&self) -> u64 {
        self.size().length
    }

    /// Returns an iterator over the list. The iterator element type is (u64, V).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.iter() {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter(&self) -> SparseListIndexIter<V> {
        SparseListIndexIter {
            base_iter: self.base.iter_from(&(), &0u64),
        }
    }

    /// Returns an iterator over the indices of the 'SparseListIndex'.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.indices() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn indices(&self) -> SparseListIndexKeys {
        SparseListIndexKeys {
            base_iter: self.base.iter_from(&(), &0u64),
        }
    }

    /// Returns an iterator over the values of the 'SparseListIndex'. The iterator element type is
    /// V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.values() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values(&self) -> SparseListIndexValues<V> {
        SparseListIndexValues {
            base_iter: self.base.iter_from(&(), &0u64),
        }
    }

    /// Returns an iterator over the list starting from the specified position. The iterator
    /// element type is (u64, V).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    /// index.remove(3);
    ///
    /// for val in index.iter_from(3) {
    ///     println!("{:?}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: u64) -> SparseListIndexIter<V> {
        SparseListIndexIter {
            base_iter: self.base.iter_from(&(), &from),
        }
    }
}

impl<'a, V> SparseListIndex<&'a mut Fork, V>
where
    V: StorageValue,
{
    fn set_size(&mut self, size: SparseListSize) {
        self.base.put(&(), size);
        self.size.set(Some(size));
    }

    /// Appends an element to the back of the 'SparseListIndex'.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    /// ```
    pub fn push(&mut self, value: V) {
        let mut size = self.size();
        self.base.put(&size.capacity, value);
        size.capacity += 1;
        size.length += 1;
        self.set_size(size);
    }

    /// Removes the element with the given index from the list and returns it,
    /// or None if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    /// assert_eq!(0, index.capacity());
    ///
    /// index.push(10);
    /// index.push(12);
    ///
    /// assert_eq!(Some(10), index.remove(0));
    /// assert_eq!(None, index.remove(0));
    /// assert_eq!(2, index.capacity());
    /// assert_eq!(1, index.len());

    /// assert_eq!(Some(12), index.remove(1));
    /// assert_eq!(2, index.capacity());
    /// ```
    pub fn remove(&mut self, index: u64) -> Option<V> {
        let mut size = self.size();
        if index >= size.capacity {
            return None;
        }
        let v = self.base.get(&index);
        if v.is_some() {
            self.base.remove(&index);
            size.length -= 1;
            self.set_size(size);
        }
        v
    }

    /// Extends the list with the contents of an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    /// assert!(index.is_empty());
    ///
    /// index.extend([1, 2, 3].iter().cloned());
    /// assert_eq!(3, index.capacity());
    /// ```
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = V>,
    {
        let mut size = self.size();
        for value in iter {
            self.base.put(&size.capacity, value);
            size.capacity += 1;
            size.length += 1;
        }
        self.set_size(size);
    }

    /// Changes a value at the specified position. If the position contains empty value it
    /// also increments elements count. If a value of the index of new element is greater than
    /// current capacity, the capacity of the list considered index + 1 and all further elements
    /// without specific index value will be appended after this index.
    ///
    /// Returns the value of an old element at this position or `None` if it was empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.get(0));
    ///
    /// index.set(0, 10);
    /// assert_eq!(Some(10), index.get(0));
    /// ```
    pub fn set(&mut self, index: u64, value: V) -> Option<V> {
        let mut size = self.size();
        // Update items count
        let old_value = self.base.get::<u64, V>(&index);
        if old_value.is_none() {
            size.length += 1;
            if index >= size.capacity {
                size.capacity = index + 1;
            }
            self.set_size(size);
        }
        self.base.put(&index, value);
        old_value
    }

    /// Clears the list, removing all values.
    ///
    /// # Notes
    ///
    /// Currently this method is not optimized to delete a large set of data.
    /// During the execution of this method the amount of allocated memory
    /// is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    ///
    /// index.clear();
    /// assert!(index.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.size.set(Some(SparseListSize::default()));
        self.base.clear()
    }

    /// Removes the first element from the 'SparseListIndex' and returns it, or None if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, SparseListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let mut fork = db.fork();
    /// let mut index = SparseListIndex::new("name", &mut fork);
    /// assert_eq!(None, index.pop());
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.pop());
    /// ```
    pub fn pop(&mut self) -> Option<V> {
        let first_item = { self.iter().next() };

        if let Some((first_index, first_elem)) = first_item {
            let mut size = self.size();
            self.base.remove(&first_index);
            size.length -= 1;
            self.set_size(size);
            return Some(first_elem);
        }
        None
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a SparseListIndex<T, V>
where
    T: AsRef<Snapshot>,
    V: StorageValue,
{
    type Item = (u64, V);
    type IntoIter = SparseListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for SparseListIndexIter<'a, V>
where
    V: StorageValue,
{
    type Item = (u64, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

impl<'a> Iterator for SparseListIndexKeys<'a> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

impl<'a, V> Iterator for SparseListIndexValues<'a, V>
where
    V: StorageValue,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(.., v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::SparseListIndex;
    use rand::{thread_rng, Rng};
    use storage::db::Database;

    const IDX_NAME: &'static str = "idx_name";

    fn gen_tempdir_name() -> String {
        thread_rng().gen_ascii_chars().take(10).collect()
    }

    fn list_index_methods(db: Box<Database>) {
        let mut fork = db.fork();
        let mut list_index = SparseListIndex::new(IDX_NAME, &mut fork);

        assert!(list_index.is_empty());
        assert_eq!(0, list_index.capacity());
        assert!(list_index.get(0).is_none());

        let extended_by = vec![45, 3422, 234];
        list_index.extend(extended_by);
        assert!(!list_index.is_empty());
        assert_eq!(Some(45), list_index.get(0));
        assert_eq!(Some(3422), list_index.get(1));
        assert_eq!(Some(234), list_index.get(2));
        assert_eq!(3, list_index.capacity());
        assert_eq!(3, list_index.len());

        assert_eq!(Some(234), list_index.set(2, 777));
        assert_eq!(Some(777), list_index.get(2));
        assert_eq!(3, list_index.capacity());
        assert_eq!(3, list_index.len());

        let extended_by_again = vec![666, 999];
        for el in &extended_by_again {
            list_index.push(*el);
        }
        assert_eq!(Some(666), list_index.get(3));
        assert_eq!(Some(999), list_index.get(4));
        assert_eq!(5, list_index.capacity());
        assert_eq!(5, list_index.len());

        assert_eq!(Some(3422), list_index.remove(1));
        assert_eq!(None, list_index.remove(1));
        assert_eq!(5, list_index.capacity());
        assert_eq!(4, list_index.len());

        assert_eq!(Some(777), list_index.remove(2));
        assert_eq!(5, list_index.capacity());
        assert_eq!(3, list_index.len());

        assert_eq!(Some(45), list_index.pop());
        assert_eq!(5, list_index.capacity());
        assert_eq!(2, list_index.len());
        assert_eq!(Some(666), list_index.pop());
        assert_eq!(5, list_index.capacity());
        assert_eq!(1, list_index.len());

        list_index.push(42);
        assert_eq!(6, list_index.capacity());
        assert_eq!(2, list_index.len());

        assert_eq!(Some(999), list_index.pop());
        assert_eq!(6, list_index.capacity());
        assert_eq!(1, list_index.len());
        assert_eq!(Some(42), list_index.pop());
        assert_eq!(6, list_index.capacity());
        assert_eq!(0, list_index.len());
        assert_eq!(None, list_index.pop());

        // check that capacity gets overwritten by bigger index correctly
        assert_eq!(None, list_index.set(42, 1024));
        assert_eq!(43, list_index.capacity());
    }

    fn list_index_iter(db: Box<Database>) {
        let mut fork = db.fork();
        let mut list_index = SparseListIndex::new(IDX_NAME, &mut fork);

        list_index.extend(vec![1u8, 15, 25, 2, 3]);
        assert_eq!(
            list_index.indices().collect::<Vec<u64>>(),
            vec![0u64, 1, 2, 3, 4]
        );
        assert_eq!(
            list_index.values().collect::<Vec<u8>>(),
            vec![1u8, 15, 25, 2, 3]
        );

        list_index.remove(1);
        list_index.remove(2);

        assert_eq!(
            list_index.iter().collect::<Vec<(u64, u8)>>(),
            vec![(0u64, 1u8), (3u64, 2u8), (4u64, 3u8)]
        );

        assert_eq!(
            list_index.iter_from(0).collect::<Vec<(u64, u8)>>(),
            vec![(0u64, 1u8), (3u64, 2u8), (4u64, 3u8)]
        );
        assert_eq!(
            list_index.iter_from(1).collect::<Vec<(u64, u8)>>(),
            vec![(3u64, 2u8), (4u64, 3u8)]
        );
        assert_eq!(
            list_index.iter_from(5).collect::<Vec<(u64, u8)>>(),
            Vec::<(u64, u8)>::new()
        );

        assert_eq!(list_index.indices().collect::<Vec<u64>>(), vec![0u64, 3, 4]);
        assert_eq!(list_index.values().collect::<Vec<u8>>(), vec![1u8, 2, 3]);
    }

    mod memorydb_tests {
        use std::path::Path;
        use storage::{Database, MemoryDB};
        use tempdir::TempDir;

        fn create_database(_: &Path) -> Box<Database> {
            Box::new(MemoryDB::new())
        }

        #[test]
        fn test_list_index_methods() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            super::list_index_methods(db);
        }

        #[test]
        fn test_list_index_iter() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            super::list_index_iter(db);
        }
    }

    mod rocksdb_tests {
        use std::path::Path;
        use storage::{Database, DbOptions, RocksDB};
        use tempdir::TempDir;

        fn create_database(path: &Path) -> Box<Database> {
            let opts = DbOptions::default();
            Box::new(RocksDB::open(path, &opts).unwrap())
        }

        #[test]
        fn test_list_index_methods() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            super::list_index_methods(db);
        }

        #[test]
        fn test_list_index_iter() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            super::list_index_iter(db);
        }
    }
}
