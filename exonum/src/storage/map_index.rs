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

//! An implementation of key-value map.

use std::{borrow::Borrow, marker::PhantomData};

use super::{
    base_index::{BaseIndex, BaseIndexIter}, indexes_metadata::IndexType, Fork, Snapshot,
    StorageKey, StorageValue,
};

/// A map of keys and values.
///
/// `MapIndex` requires that the keys implement the [`StorageKey`] trait and the values implement
/// [`StorageValue`] trait.
///
/// [`StorageKey`]: ../trait.StorageKey.html
/// [`StorageValue`]: ../trait.StorageValue.html
#[derive(Debug)]
pub struct MapIndex<T, K, V> {
    base: BaseIndex<T>,
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
pub struct MapIndexIter<'a, K, V> {
    base_iter: BaseIndexIter<'a, K, V>,
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
pub struct MapIndexKeys<'a, K> {
    base_iter: BaseIndexIter<'a, K, ()>,
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
pub struct MapIndexValues<'a, V> {
    base_iter: BaseIndexIter<'a, (), V>,
}

impl<T, K, V> MapIndex<T, K, V>
where
    T: AsRef<Snapshot>,
    K: StorageKey,
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
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new(name, &snapshot);
    /// ```
    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {
        MapIndex {
            base: BaseIndex::new(index_name, IndexType::Map, view),
            _k: PhantomData,
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
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let index_id = vec![01];
    ///
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new_in_family(name, &index_id, &snapshot);
    /// ```
    pub fn new_in_family<S: AsRef<str>, I: StorageKey>(
        family_name: S,
        index_id: &I,
        view: T,
    ) -> Self {
        MapIndex {
            base: BaseIndex::new_in_family(family_name, index_id, IndexType::Map, view),
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Returns a value corresponding to the key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = MapIndex::new(name, &mut fork);
    /// assert!(index.get(&1).is_none());
    ///
    /// index.put(&1, 2);
    /// assert_eq!(Some(2), index.get(&1));
    /// ```
    pub fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: StorageKey + ?Sized,
    {
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
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = MapIndex::new(name, &mut fork);
    /// assert!(!index.contains(&1));
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: StorageKey + ?Sized,
    {
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
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new(name, &snapshot);
    ///
    /// for v in index.iter() {
    ///     println!("{:?}", v);
    /// }
    /// ```
    pub fn iter(&self) -> MapIndexIter<K, V> {
        MapIndexIter {
            base_iter: self.base.iter(&()),
        }
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
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new(name, &snapshot);
    ///
    /// for key in index.keys() {
    ///     println!("{}", key);
    /// }
    /// ```
    pub fn keys(&self) -> MapIndexKeys<K> {
        MapIndexKeys {
            base_iter: self.base.iter(&()),
        }
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
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new(name, &snapshot);
    ///
    /// for val in index.values() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values(&self) -> MapIndexValues<V> {
        MapIndexValues {
            base_iter: self.base.iter(&()),
        }
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
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new(name, &snapshot);
    ///
    /// for v in index.iter_from(&2) {
    ///     println!("{:?}", v);
    /// }
    /// ```
    pub fn iter_from<Q>(&self, from: &Q) -> MapIndexIter<K, V>
    where
        K: Borrow<Q>,
        Q: StorageKey + ?Sized,
    {
        MapIndexIter {
            base_iter: self.base.iter_from(&(), from),
        }
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
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new(name, &snapshot);
    ///
    /// for key in index.keys_from(&2) {
    ///     println!("{}", key);
    /// }
    /// ```
    pub fn keys_from<Q>(&self, from: &Q) -> MapIndexKeys<K>
    where
        K: Borrow<Q>,
        Q: StorageKey + ?Sized,
    {
        MapIndexKeys {
            base_iter: self.base.iter_from(&(), from),
        }
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
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new(name, &snapshot);
    /// for val in index.values_from(&2) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values_from<Q>(&self, from: &Q) -> MapIndexValues<V>
    where
        K: Borrow<Q>,
        Q: StorageKey + ?Sized,
    {
        MapIndexValues {
            base_iter: self.base.iter_from(&(), from),
        }
    }
}

impl<'a, K, V> MapIndex<&'a mut Fork, K, V>
where
    K: StorageKey,
    V: StorageValue,
{
    /// Inserts the key-value pair into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, MapIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = MapIndex::new(name, &mut fork);
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
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = MapIndex::new(name, &mut fork);
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    ///
    /// index.remove(&1);
    /// assert!(!index.contains(&1));
    pub fn remove<Q>(&mut self, key: &Q)
    where
        K: Borrow<Q>,
        Q: StorageKey + ?Sized,
    {
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
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = MapIndex::new(name, &mut fork);
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

impl<'a, T, K, V> ::std::iter::IntoIterator for &'a MapIndex<T, K, V>
where
    T: AsRef<Snapshot>,
    K: StorageKey,
    V: StorageValue,
{
    type Item = (K::Owned, V);
    type IntoIter = MapIndexIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> Iterator for MapIndexIter<'a, K, V>
where
    K: StorageKey,
    V: StorageValue,
{
    type Item = (K::Owned, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

impl<'a, K> Iterator for MapIndexKeys<'a, K>
where
    K: StorageKey,
{
    type Item = K::Owned;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

impl<'a, V> Iterator for MapIndexValues<'a, V>
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
    use super::super::{Database, MemoryDB};
    use super::*;
    use rand::{thread_rng, Rng};

    const IDX_NAME: &'static str = "idx_name";

    #[test]
    fn str_key() {
        let db = MemoryDB::new();
        let mut fork = db.fork();

        const KEY: &str = "key_1";

        let mut index: MapIndex<_, String, _> = MapIndex::new(IDX_NAME, &mut fork);
        assert_eq!(false, index.contains(KEY));

        index.put(&KEY.to_owned(), 0);
        assert_eq!(true, index.contains(KEY));

        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    #[test]
    fn u8_slice_key() {
        let db = MemoryDB::new();
        let mut fork = db.fork();

        const KEY: &[u8] = &[1, 2, 3];

        let mut index: MapIndex<_, Vec<u8>, _> = MapIndex::new(IDX_NAME, &mut fork);
        assert_eq!(false, index.contains(KEY));

        index.put(&KEY.to_owned(), 0);
        assert_eq!(true, index.contains(KEY));

        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    fn iter(db: Box<Database>) {
        let mut fork = db.fork();
        let mut map_index = MapIndex::new(IDX_NAME, &mut fork);

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

        map_index.remove(&1u8);
        assert_eq!(
            map_index.iter_from(&0u8).collect::<Vec<(u8, u8)>>(),
            vec![(2, 2), (3, 3)]
        );
        assert_eq!(
            map_index.iter_from(&1u8).collect::<Vec<(u8, u8)>>(),
            vec![(2, 2), (3, 3)]
        );
    }

    fn gen_tempdir_name() -> String {
        thread_rng().gen_ascii_chars().take(10).collect()
    }

    mod memorydb_tests {
        use std::path::Path;
        use storage::{Database, MemoryDB};
        use tempdir::TempDir;

        fn create_database(_: &Path) -> Box<Database> {
            Box::new(MemoryDB::new())
        }

        #[test]
        fn test_iter() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            super::iter(db);
        }

    }

    mod rocksdb_tests {
        use std::path::Path;
        use storage::Database;
        use tempdir::TempDir;

        fn create_database(path: &Path) -> Box<Database> {
            use storage::{DbOptions, RocksDB};
            let opts = DbOptions::default();
            Box::new(RocksDB::open(path, &opts).unwrap())
        }

        #[test]
        fn test_iter() {
            let dir = TempDir::new(super::gen_tempdir_name().as_str()).unwrap();
            let path = dir.path();
            let db = create_database(path);
            super::iter(db);
        }
    }
}
