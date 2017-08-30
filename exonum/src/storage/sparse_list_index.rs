// Copyright 2017 The Exonum Team
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

use byteorder::{BigEndian, ByteOrder};

use std::borrow::Cow;
use std::cell::Cell;
use std::marker::PhantomData;

use crypto::{hash, Hash};
use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageValue};

#[derive(Debug, Default, Clone, Copy)]
struct SparseListSize {
    /// Total list length including spaces.
    total_length: u64,
    /// Number of non-empty elements.
    items_count: u64,
}

impl SparseListSize {
    fn zero() -> SparseListSize {
        SparseListSize::default()
    }

    fn into_array(&self) -> [u8; 16] {
        let mut buf = [0; 16];
        BigEndian::write_u64(&mut buf[0..8], self.total_length);
        BigEndian::write_u64(&mut buf[8..16], self.items_count);
        buf
    }
}

impl StorageValue for SparseListSize {
    fn hash(&self) -> Hash {
        hash(&self.into_array())
    }

    fn into_bytes(self) -> Vec<u8> {
        self.into_array().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        let buf = value.as_ref();
        let total_length = BigEndian::read_u64(&buf[0..8]);
        let items_count = BigEndian::read_u64(&buf[8..16]);
        SparseListSize {
            total_length,
            items_count,
        }
    }
}

/// TODO
#[derive(Debug)]
pub struct SparseListIndex<T, V> {
    base: BaseIndex<T>,
    size: Cell<Option<SparseListSize>>,
    _v: PhantomData<V>,
}

/// TODO
#[derive(Debug)]
pub struct SparseListIndexIter<'a, V> {
    base_iter: BaseIndexIter<'a, u64, V>,
}

impl<T, V> SparseListIndex<T, V> {
    /// TODO
    pub fn new(prefix: Vec<u8>, view: T) -> Self {
        SparseListIndex {
            base: BaseIndex::new(prefix, view),
            size: Cell::new(None),
            _v: PhantomData,
        }
    }
}

impl<T, V> SparseListIndex<T, V>
where
    T: AsRef<Snapshot>,
    V: StorageValue,
{
    fn size(&self) -> SparseListSize {
        if let Some(size) = self.size.get() {
            return size;
        }
        let size = self.base.get(&()).unwrap_or_default();
        self.size.set(Some(size));
        size
    }

    /// TODO
    pub fn get(&self, index: u64) -> Option<V> {
        self.base.get(&index)
    }

    /// TODO
    pub fn last(&self) -> Option<V> {
        match self.len() {
            0 => None,
            l => self.get(l - 1),
        }
    }

    /// TODO
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// TODO
    pub fn len(&self) -> u64 {
        self.size().total_length
    }

    /// TODO
    pub fn count(&self) -> u64 {
        self.size().items_count
    }

    /// TODO
    pub fn iter(&self) -> SparseListIndexIter<V> {
        SparseListIndexIter { base_iter: self.base.iter_from(&(), &0u64) }
    }

    // FIXME iter_from disabled because it is not usable.
    // pub fn iter_from(&self, from: u64) -> SparseListIndexIter<V> {
    //     SparseListIndexIter { base_iter: self.base.iter_from(&(), &from) }
    // }
}


impl<'a, V> SparseListIndex<&'a mut Fork, V>
where
    V: StorageValue,
{
    fn set_size(&mut self, size: SparseListSize) {
        self.base.put(&(), size);
        self.size.set(Some(size));
    }

    /// TODO
    pub fn push(&mut self, value: V) {
        let mut size = self.size();
        self.base.put(&size.total_length, value);
        size.total_length += 1;
        size.items_count += 1;
        self.set_size(size);
    }

    /// TODO
    pub fn pop(&mut self) -> Option<V> {
        // TODO: shoud we get and return dropped value?
        let mut size = self.size();
        match size.total_length {
            0 => None,
            l => {
                let v = self.base.get(&(l - 1));
                self.base.remove(&(l - 1));
                size.total_length -= 1;
                size.items_count -= 1;
                self.set_size(size);
                v
            }
        }
    }

    /// TODO
    pub fn remove(&mut self, index: u64) -> Option<V> {
        let mut size = self.size();
        if index >= size.total_length {
            panic!(
                "index out of bounds: \
                    the len is {} but the index is {}",
                size.total_length,
                index
            );
        }
        let v = self.base.get(&index);
        if v.is_some() {
            self.base.remove(&index);
            if index == size.total_length - 1 {
                size.total_length -= 1;
            }
            size.items_count -= 1;
            self.set_size(size);
        }
        v
    }

    /// TODO
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = V>,
    {
        let mut size = self.size();
        for value in iter {
            self.base.put(&size.total_length, value);
            size.total_length += 1;
            size.items_count += 1;
        }
        self.set_size(size);
    }

    /// TODO
    pub fn truncate(&mut self, len: u64) {
        // TODO: optimize this
        while self.len() > len {
            self.pop();
        }
    }

    /// TODO
    pub fn set(&mut self, index: u64, value: V) {
        let mut size = self.size();
        if index >= size.total_length {
            panic!(
                "index out of bounds: \
                    the len is {} but the index is {}",
                size.total_length,
                index
            );
        }
        // Increment items count
        if self.base.get::<u64, V>(&index).is_none() {
            size.items_count += 1;
            self.set_size(size);
        }
        self.base.put(&index, value)
    }

    /// TODO
    pub fn clear(&mut self) {
        self.size.set(Some(SparseListSize::zero()));
        self.base.clear()
    }
}


impl<'a, T, V> ::std::iter::IntoIterator for &'a SparseListIndex<T, V>
where
    T: AsRef<Snapshot>,
    V: StorageValue,
{
    type Item = V;
    type IntoIter = SparseListIndexIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> Iterator for SparseListIndexIter<'a, V>
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
    use rand::{thread_rng, Rng};
    use super::SparseListIndex;
    use storage::db::Database;

    fn gen_tempdir_name() -> String {
        thread_rng().gen_ascii_chars().take(10).collect()
    }

    fn list_index_methods(db: Box<Database>) {
        let mut fork = db.fork();
        let mut list_index = SparseListIndex::new(vec![255], &mut fork);

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
        assert_eq!(3, list_index.count());

        list_index.set(2, 777);
        assert_eq!(Some(777), list_index.get(2));
        assert_eq!(Some(777), list_index.last());
        assert_eq!(3, list_index.len());
        assert_eq!(3, list_index.count());

        let mut extended_by_again = vec![666, 999];
        for el in &extended_by_again {
            list_index.push(*el);
        }
        assert_eq!(Some(666), list_index.get(3));
        assert_eq!(Some(999), list_index.get(4));
        assert_eq!(5, list_index.len());
        assert_eq!(5, list_index.count());
        extended_by_again[1] = 1001;
        list_index.extend(extended_by_again);
        assert_eq!(7, list_index.len());
        assert_eq!(7, list_index.count());
        assert_eq!(Some(1001), list_index.last());

        assert_eq!(Some(1001), list_index.pop());
        assert_eq!(6, list_index.len());
        assert_eq!(6, list_index.count());

        list_index.truncate(3);

        assert_eq!(3, list_index.len());
        assert_eq!(Some(777), list_index.last());

        assert_eq!(Some(3422), list_index.remove(1));
        assert_eq!(None, list_index.remove(1));
        assert_eq!(3, list_index.len());
        assert_eq!(2, list_index.count());

        assert_eq!(Some(777), list_index.remove(2));
        assert_eq!(2, list_index.len());
        assert_eq!(1, list_index.count());
    }

    fn list_index_iter(db: Box<Database>) {
        let mut fork = db.fork();
        let mut list_index = SparseListIndex::new(vec![255], &mut fork);

        list_index.extend(vec![1u8, 15, 25, 2, 3]);
        list_index.remove(1);
        list_index.remove(2);

        assert_eq!(list_index.iter().collect::<Vec<u8>>(), vec![1, 2, 3]);

        // assert_eq!(list_index.iter_from(0).collect::<Vec<u8>>(), vec![1, 2, 3]);
        // assert_eq!(list_index.iter_from(3).collect::<Vec<u8>>(), vec![2, 3]);
        // assert_eq!(
        //     list_index.iter_from(5).collect::<Vec<u8>>(),
        //     Vec::<u8>::new()
        // );
    }

    mod memorydb_tests {
        use std::path::Path;
        use tempdir::TempDir;
        use storage::{Database, MemoryDB};

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

    #[cfg(feature = "leveldb")]
    mod leveldb_tests {
        use std::path::Path;
        use tempdir::TempDir;
        use storage::{Database, LevelDB, LevelDBOptions};

        fn create_database(path: &Path) -> Box<Database> {
            let mut opts = LevelDBOptions::default();
            opts.create_if_missing = true;
            Box::new(LevelDB::open(path, opts).unwrap())
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

    #[cfg(feature = "rocksdb")]
    mod rocksdb_tests {
        use std::path::Path;
        use tempdir::TempDir;
        use storage::{Database, RocksDB, RocksDBOptions};

        fn create_database(path: &Path) -> Box<Database> {
            let mut opts = RocksDBOptions::default();
            opts.create_if_missing(true);
            Box::new(RocksDB::open(path, opts).unwrap())
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
