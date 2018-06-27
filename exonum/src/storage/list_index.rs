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

//! An implementation of array list of items.

use std::{cell::Cell, marker::PhantomData};

use super::{
    base_index::{BaseIndex, BaseIndexIter}, indexes_metadata::IndexType, Fork, Snapshot,
    StorageKey, StorageValue,
};

/// A list of items that implement `StorageValue` trait.
///
/// `ListIndex` implements an array list, storing the element as values and using `u64` as an index.
/// `ListIndex` requires that the elements implement the [`StorageValue`] trait.
///
/// [`StorageValue`]: ../trait.StorageValue.html
#[derive(Debug)]
pub struct ListIndex<T, V> {
    base: BaseIndex<T>,
    length: Cell<Option<u64>>,
    _v: PhantomData<V>,
}

/// An iterator over the items of a `ListIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`ListIndex`]. See its documentation for more.
///
/// [`iter`]: struct.ListIndex.html#method.iter
/// [`iter_from`]: struct.ListIndex.html#method.iter_from
/// [`ListIndex`]: struct.ListIndex.html
#[derive(Debug)]
pub struct ListIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, u64, V>,
}

impl<T, V> ListIndex<T, V>
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
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: ListIndex<_, u8> = ListIndex::new(name, &snapshot);
    /// ```
    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {
        ListIndex {
            base: BaseIndex::new(index_name, IndexType::List, view),
            length: Cell::new(None),
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
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let index_id = vec![01];
    /// let snapshot = db.snapshot();
    /// let index: ListIndex<_, u8> = ListIndex::new_in_family(name, &index_id, &snapshot);
    /// ```
    pub fn new_in_family<S: AsRef<str>, I: StorageKey>(
        family_name: S,
        index_id: &I,
        view: T,
    ) -> Self {
        ListIndex {
            base: BaseIndex::new_in_family(family_name, index_id, IndexType::List, view),
            length: Cell::new(None),
            _v: PhantomData,
        }
    }

    /// Returns an element at that position or `None` if out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    /// assert_eq!(None, index.get(0));
    ///
    /// index.push(42);
    /// assert_eq!(Some(42), index.get(0));
    /// ```
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&index)
    }

    /// Returns the last element of the list, or `None` if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    /// assert_eq!(None, index.last());
    ///
    /// index.push(42);
    /// assert_eq!(Some(42), index.last());
    /// ```
    pub fn last(&self) -> Option<V> {
        match self.len() {
            0 => None,
            l => self.get(l - 1),
        }
    }

    /// Returns `true` if the list contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    /// assert!(index.is_empty());
    ///
    /// index.push(42);
    /// assert!(!index.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of elements in the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    /// assert_eq!(0, index.len());
    ///
    /// index.push(10);
    /// assert_eq!(1, index.len());
    ///
    /// index.push(100);
    /// assert_eq!(2, index.len());
    /// ```
    pub fn len(&self) -> u64 {
        if let Some(len) = self.length.get() {
            return len;
        }
        let len = self.base.get(&()).unwrap_or(0);
        self.length.set(Some(len));
        len
    }

    /// Returns an iterator over the list. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.iter() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter(&self) -> ListIndexIter<V> {
        ListIndexIter {
            base_iter: self.base.iter_from(&(), &0u64),
        }
    }

    /// Returns an iterator over the list starting from the specified position. The iterator
    /// element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    ///
    /// for val in index.iter_from(3) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: u64) -> ListIndexIter<V> {
        ListIndexIter {
            base_iter: self.base.iter_from(&(), &from),
        }
    }
}

impl<'a, V> ListIndex<&'a mut Fork, V>
where
    V: StorageValue,
{
    fn set_len(&mut self, len: u64) {
        self.base.put(&(), len);
        self.length.set(Some(len));
    }

    /// Appends an element to the back of the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    /// ```
    pub fn push(&mut self, value: V) {
        let len = self.len();
        self.base.put(&len, value);
        self.set_len(len + 1)
    }

    /// Removes the last element from the list and returns it, or None if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    /// assert_eq!(None, index.pop());
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.pop());
    /// ```
    pub fn pop(&mut self) -> Option<V> {
        match self.len() {
            0 => None,
            l => {
                let v = self.base.get(&(l - 1));
                self.base.remove(&(l - 1));
                self.set_len(l - 1);
                v
            }
        }
    }

    /// Extends the list with the contents of an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    /// assert!(index.is_empty());
    ///
    /// index.extend([1, 2, 3].iter().cloned());
    /// assert_eq!(3, index.len());
    /// ```
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = V>,
    {
        let mut len = self.len();
        for value in iter {
            self.base.put(&len, value);
            len += 1;
        }
        self.base.put(&(), len);
        self.set_len(len);
    }

    /// Shortens the list, keeping the first `len` elements and dropping the rest.
    ///
    /// If `len` is greater than the list's current length, this has no effect.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    ///
    /// index.extend([1, 2, 3, 4, 5].iter().cloned());
    /// assert_eq!(5, index.len());
    ///
    /// index.truncate(3);
    /// assert_eq!(3, index.len());
    /// ```
    pub fn truncate(&mut self, len: u64) {
        // TODO: Optimize this. (ECR-175)
        while self.len() > len {
            self.pop();
        }
    }

    /// Changes a value at the specified position.
    ///
    /// # Panics
    ///
    /// Panics if `index` is equal or greater than the list's current length.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    ///
    /// index.push(1);
    /// assert_eq!(Some(1), index.get(0));
    ///
    /// index.set(0, 10);
    /// assert_eq!(Some(10), index.get(0));
    /// ```
    pub fn set(&mut self, index: u64, value: V) {
        if index >= self.len() {
            panic!(
                "index out of bounds: \
                 the len is {} but the index is {}",
                self.len(),
                index
            );
        }
        self.base.put(&index, value)
    }

    /// Clears the list, removing all values.
    ///
    /// # Notes
    ///
    /// Currently this method is not optimized to delete large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, ListIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = ListIndex::new(name, &mut fork);
    ///
    /// index.push(1);
    /// assert!(!index.is_empty());
    ///
    /// index.clear();
    /// assert!(index.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.length.set(Some(0));
        self.base.clear()
    }
}

impl<'a, T, V> ::std::iter::IntoIterator for &'a ListIndex<T, V>
where
    T: AsRef<Snapshot>,
    V: StorageValue,
{
    type Item = V;
    type IntoIter = ListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for ListIndexIter<'a, V>
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
    use super::{Fork, ListIndex};
    use rand::{thread_rng, Rng};

    fn gen_tempdir_name() -> String {
        thread_rng().gen_ascii_chars().take(10).collect()
    }

    fn list_index_methods(list_index: &mut ListIndex<&mut Fork, i32>) {
        assert!(list_index.is_empty());
        assert_eq!(0, list_index.len());
        assert!(list_index.last().is_none());

        let extended_by = vec![45, 3422, 234];
        list_index.extend(extended_by);
        assert!(!list_index.is_empty());
        assert_eq!(Some(45), list_index.get(0));
        assert_eq!(Some(3422), list_index.get(1));
        assert_eq!(Some(234), list_index.get(2));
        assert_eq!(3, list_index.len());

        list_index.set(2, 777);
        assert_eq!(Some(777), list_index.get(2));
        assert_eq!(Some(777), list_index.last());
        assert_eq!(3, list_index.len());

        let mut extended_by_again = vec![666, 999];
        for el in &extended_by_again {
            list_index.push(*el);
        }
        assert_eq!(Some(666), list_index.get(3));
        assert_eq!(Some(999), list_index.get(4));
        assert_eq!(5, list_index.len());
        extended_by_again[1] = 1001;
        list_index.extend(extended_by_again);
        assert_eq!(7, list_index.len());
        assert_eq!(Some(1001), list_index.last());

        assert_eq!(Some(1001), list_index.pop());
        assert_eq!(6, list_index.len());

        list_index.truncate(3);

        assert_eq!(3, list_index.len());
        assert_eq!(Some(777), list_index.last());
    }

    fn list_index_iter(list_index: &mut ListIndex<&mut Fork, u8>) {
        list_index.extend(vec![1u8, 2, 3]);

        assert_eq!(list_index.iter().collect::<Vec<u8>>(), vec![1, 2, 3]);

        assert_eq!(list_index.iter_from(0).collect::<Vec<u8>>(), vec![1, 2, 3]);
        assert_eq!(list_index.iter_from(1).collect::<Vec<u8>>(), vec![2, 3]);
        assert_eq!(
            list_index.iter_from(3).collect::<Vec<u8>>(),
            Vec::<u8>::new()
        );
    }

    mod memorydb_tests {
        use std::path::Path;
        use storage::{Database, ListIndex, MemoryDB};
        use tempdir::TempDir;

        const IDX_NAME: &'static str = "idx_name";

        fn create_database(_: &Path) -> Box<Database> {
            Box::new(MemoryDB::new())
        }

        #[test]
        fn test_list_index_methods() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            let mut fork = db.fork();
            let mut list_index = ListIndex::new(IDX_NAME, &mut fork);
            super::list_index_methods(&mut list_index);
        }

        #[test]
        fn test_list_index_in_family_methods() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            let mut fork = db.fork();
            let mut list_index = ListIndex::new_in_family(IDX_NAME, &vec![01], &mut fork);
            super::list_index_methods(&mut list_index);
        }

        #[test]
        fn test_list_index_iter() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            let mut fork = db.fork();
            let mut list_index = ListIndex::new(IDX_NAME, &mut fork);
            super::list_index_iter(&mut list_index);
        }

        #[test]
        fn test_list_index_in_family_iter() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            let mut fork = db.fork();
            let mut list_index = ListIndex::new_in_family(IDX_NAME, &vec![01], &mut fork);
            super::list_index_iter(&mut list_index);
        }
    }

    mod rocksdb_tests {
        use std::path::Path;
        use storage::{Database, DbOptions, ListIndex, RocksDB};
        use tempdir::TempDir;

        const IDX_NAME: &'static str = "idx_name";

        fn create_database(path: &Path) -> Box<Database> {
            let opts = DbOptions::default();
            Box::new(RocksDB::open(path, &opts).unwrap())
        }

        #[test]
        fn test_list_index_methods() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            let mut fork = db.fork();
            let mut list_index = ListIndex::new(IDX_NAME, &mut fork);
            super::list_index_methods(&mut list_index);
        }

        #[test]
        fn test_list_index_in_family_methods() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            let mut fork = db.fork();
            let mut list_index = ListIndex::new_in_family(IDX_NAME, &vec![01], &mut fork);
            super::list_index_methods(&mut list_index);
        }

        #[test]
        fn test_list_index_iter() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            let mut fork = db.fork();
            let mut list_index = ListIndex::new(IDX_NAME, &mut fork);
            super::list_index_iter(&mut list_index);
        }

        #[test]
        fn test_list_index_in_family_iter() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            let mut fork = db.fork();
            let mut list_index = ListIndex::new_in_family(IDX_NAME, &vec![01], &mut fork);
            super::list_index_iter(&mut list_index);
        }
    }
}
