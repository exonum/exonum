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

//! An implementation of key-value map.
use std::sync::Arc;
use std::marker::PhantomData;

use super::{BaseIndex, BaseIndexIter, View, StorageKey, StorageValue};

/// A map of keys and values.
///
/// `MapIndex` requires that the keys implement the [`StorageKey`] trait and the values implement
/// [`StorageValue`] trait.
/// [`StorageKey`]: ../trait.StorageKey.html
/// [`StorageValue`]: ../trait.StorageValue.html
#[derive(Debug)]
pub struct MapIndex<K, V> {
    base: BaseIndex,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

/// An iterator over the entries of a `MapIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`MapIndex`]. See its documentation for more.
///
/// [`iter`]: struct.MapIndex.html#method.iter
/// [`iter_from`]: struct.MapIndex.html#method.iter_from
/// [`MapIndex`]: struct.MapIndex.html
#[derive(Debug)]
pub struct MapIndexIter<K, V> {
    base_iter: BaseIndexIter<K, V>,
}

/// An iterator over the keys of a `MapIndex`.
///
/// This struct is created by the [`keys`] or
/// [`keys_from`] methods on [`MapIndex`]. See its documentation for more.
///
/// [`keys`]: struct.MapIndex.html#method.keys
/// [`keys_from`]: struct.MapIndex.html#method.keys_from
/// [`MapIndex`]: struct.MapIndex.html
#[derive(Debug)]
pub struct MapIndexKeys<K> {
    base_iter: BaseIndexIter<K, ()>,
}

/// An iterator over the values of a `MapIndex`.
///
/// This struct is created by the [`values`] or
/// [`values_from`] methods on [`MapIndex`]. See its documentation for more.
///
/// [`values`]: struct.MapIndex.html#method.values
/// [`values_from`]: struct.MapIndex.html#method.values_from
/// [`MapIndex`]: struct.MapIndex.html
#[derive(Debug)]
pub struct MapIndexValues<V> {
    base_iter: BaseIndexIter<(), V>,
}

impl<K, V> MapIndex<K, V> {
    /// Creates a new index representation based on the common prefix of its keys and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<u8, u8> = MapIndex::new("abc", snapshot);
    /// # drop(index);
    /// ```
    pub fn new(name: &str, view: Arc<View>) -> Self {
        MapIndex {
            base: BaseIndex::new(name, view),
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<K, V> MapIndex<K, V>
where
    K: StorageKey,
    V: StorageValue,
{
    /// Returns a value corresponding to the key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let fork = db.fork();
    /// let mut index = MapIndex::new("abc", fork);
    /// assert!(index.get(&1).is_none());
    ///
    /// index.put(&1, 2);
    /// assert_eq!(Some(2), index.get(&1));
    /// ```
    pub fn get(&self, key: &K) -> Option<V> {
        self.base.get(key)
    }

    /// Returns `true` if the map contains a value for the specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let fork = db.fork();
    /// let mut index = MapIndex::new("abc", fork);
    /// assert!(!index.contains(&1));
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    pub fn contains(&self, key: &K) -> bool {
        self.base.contains(key)
    }

    /// Returns an iterator over the entries of the map in ascending order. The iterator element
    /// type is (K, V).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<u8, u8> = MapIndex::new("abc", snapshot);
    ///
    /// for v in index.iter() {
    ///     println!("{:?}", v);
    /// }
    /// ```
    pub fn iter(&self) -> MapIndexIter<K, V> {
        MapIndexIter { base_iter: self.base.iter() }
    }

    /// Returns an iterator over the keys of the map in ascending order. The iterator element
    /// type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<u8, u8> = MapIndex::new("abc", snapshot);
    ///
    /// for key in index.keys() {
    ///     println!("{}", key);
    /// }
    /// ```
    pub fn keys(&self) -> MapIndexKeys<K> {
        MapIndexKeys { base_iter: self.base.iter() }
    }

    /// Returns an iterator over the values of the map in ascending order of keys. The iterator
    /// element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<u8, u8> = MapIndex::new("abc", snapshot);
    ///
    /// for val in index.values() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values(&self) -> MapIndexValues<V> {
        MapIndexValues { base_iter: self.base.iter() }
    }

    /// Returns an iterator over the entries of the map in ascending order starting from the
    /// specified key. The iterator element type is (K, V).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<u8, u8> = MapIndex::new("abc", snapshot);
    ///
    /// for v in index.iter_from(&2) {
    ///     println!("{:?}", v);
    /// }
    /// ```
    pub fn iter_from(&self, from: &K) -> MapIndexIter<K, V> {
        MapIndexIter { base_iter: self.base.iter_from(from) }
    }

    /// Returns an iterator over the keys of the map in ascending order starting from the
    /// specified key. The iterator element type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<u8, u8> = MapIndex::new("abc", snapshot);
    ///
    /// for key in index.keys_from(&2) {
    ///     println!("{}", key);
    /// }
    /// ```
    pub fn keys_from(&self, from: &K) -> MapIndexKeys<K> {
        MapIndexKeys { base_iter: self.base.iter_from(from) }
    }

    /// Returns an iterator over the values of the map in ascending order of keys starting from the
    /// specified key. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<u8, u8> = MapIndex::new("abc", snapshot);
    //
    /// for val in index.values_from(&2) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values_from(&self, from: &K) -> MapIndexValues<V> {
        MapIndexValues { base_iter: self.base.iter_from(from) }
    }

    /// Inserts the key-value pair into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let fork = db.fork();
    /// let mut index = MapIndex::new("abc", fork);
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    pub fn put(&mut self, key: &K, value: V) {
        self.base.put(key, value)
    }

    /// Removes the key from the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let fork = db.fork();
    /// let mut index = MapIndex::new("abc", fork);
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    ///
    /// index.remove(&1);
    /// assert!(!index.contains(&1));
    pub fn remove(&mut self, key: &K) {
        self.base.remove(key)
    }

    /// Clears the map, removing all entries.
    ///
    /// # Notes
    /// Currently this method is not optimized to delete large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let fork = db.fork();
    /// let mut index = MapIndex::new("abc", fork);
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    ///
    /// index.clear();
    /// assert!(!index.contains(&1));
    pub fn clear(&mut self) {
        self.base.clear()
    }
}

impl<K, V> ::std::iter::IntoIterator for MapIndex<K, V>
where
    K: StorageKey,
    V: StorageValue,
{
    type Item = (K, V);
    type IntoIter = MapIndexIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K, V> Iterator for MapIndexIter<K, V>
where
    K: StorageKey,
    V: StorageValue,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

impl<K> Iterator for MapIndexKeys<K>
where
    K: StorageKey,
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

impl<V> Iterator for MapIndexValues<V>
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
    use storage::Database;
    use super::MapIndex;
    use rand::{thread_rng, Rng};

    fn iter(db: Box<Database>) {
        let fork = db.fork();
        let mut map_index = MapIndex::new("a", fork.clone());

        map_index.put(&1u8, 1u8);
        map_index.put(&2u8, 2u8);
        map_index.put(&3u8, 3u8);

        assert_eq!(
            map_index.iter().collect::<Vec<(u8, u8)>>(),
            vec![(1, 1), (2, 2), (3, 3)]
        );

        assert_eq!(
            map_index.iter_from(&0).collect::<Vec<(u8, u8)>>(),
            vec![(1, 1), (2, 2), (3, 3)]
        );
        assert_eq!(
            map_index.iter_from(&1).collect::<Vec<(u8, u8)>>(),
            vec![(1, 1), (2, 2), (3, 3)]
        );
        assert_eq!(
            map_index.iter_from(&2).collect::<Vec<(u8, u8)>>(),
            vec![(2, 2), (3, 3)]
        );
        assert_eq!(
            map_index.iter_from(&4).collect::<Vec<(u8, u8)>>(),
            Vec::<(u8, u8)>::new()
        );

        assert_eq!(map_index.keys().collect::<Vec<u8>>(), vec![1, 2, 3]);

        assert_eq!(map_index.keys_from(&0).collect::<Vec<u8>>(), vec![1, 2, 3]);
        assert_eq!(map_index.keys_from(&1).collect::<Vec<u8>>(), vec![1, 2, 3]);
        assert_eq!(map_index.keys_from(&2).collect::<Vec<u8>>(), vec![2, 3]);
        assert_eq!(
            map_index.keys_from(&4).collect::<Vec<u8>>(),
            Vec::<u8>::new()
        );

        assert_eq!(map_index.values().collect::<Vec<u8>>(), vec![1, 2, 3]);

        assert_eq!(
            map_index.values_from(&0).collect::<Vec<u8>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            map_index.values_from(&1).collect::<Vec<u8>>(),
            vec![1, 2, 3]
        );
        assert_eq!(map_index.values_from(&2).collect::<Vec<u8>>(), vec![2, 3]);
        assert_eq!(
            map_index.values_from(&4).collect::<Vec<u8>>(),
            Vec::<u8>::new()
        );
    }

    fn gen_tempdir_name() -> String {
        thread_rng().gen_ascii_chars().take(20).collect()
    }

    mod memorydb_tests {
        use storage::{Database, MemoryDB};

        fn create_database() -> Box<Database> {
            Box::new(MemoryDB::new())
        }

        #[test]
        fn test_iter() {
            let db = create_database();
            super::iter(db);
        }
    }

    mod rocksdb_tests {
        use std::sync::Arc;
        use std::path::Path;
        use storage::{Database, MapIndex};
        use tempdir::TempDir;

        fn create_database(path: &Path) -> Box<Database> {
            use storage::{RocksDB, RocksDBOptions};
            let mut opts = RocksDBOptions::default();
            opts.create_if_missing(true);
            Box::new(RocksDB::open(path, opts).unwrap())
        }

        #[test]
        fn test_iter() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            super::iter(db);
        }

        #[test]
        fn test_fork_and_snapshot_isolation() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            let fork = db.fork();
            {
                let mut idx = MapIndex::new("a", Arc::clone(&fork));
                idx.put(&1, 1);
            }
            {
                let snapshot = db.snapshot();
                let idx: MapIndex<i32, i32> = MapIndex::new("a", Arc::clone(&snapshot));
                assert!(!idx.contains(&1));
            }
            fork.commit();
            {
                let snapshot = db.snapshot();
                let idx: MapIndex<i32, i32> = MapIndex::new("a", Arc::clone(&snapshot));
                assert!(idx.contains(&1));
            }
        }
    }
}
