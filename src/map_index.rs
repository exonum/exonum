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

//! An implementation of a key-value map.
//!
//! `MapIndex` requires that keys implement the [`BinaryKey`] trait and values implement
//! the [`BinaryValue`] trait. The given section contains methods related to
//! `MapIndex` and iterators over the items of this map.

use std::{borrow::Borrow, marker::PhantomData};

use super::{
    base_index::{BaseIndex, BaseIndexIter},
    indexes_metadata::IndexType,
    BinaryKey, BinaryValue, Fork, Snapshot,
};

/// A map of keys and values. Access to the elements of this map is obtained using the keys.
///
/// `MapIndex` requires that keys implement the [`BinaryKey`] trait and values implement
/// the [`BinaryValue`] trait.
///
/// [`BinaryKey`]: ../trait.BinaryKey.html
/// [`BinaryValue`]: ../trait.BinaryValue.html
#[derive(Debug)]
pub struct MapIndex<T, K, V> {
    base: BaseIndex<T>,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

/// Returns an iterator over the entries of a `MapIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] method on [`MapIndex`]. See its documentation for additional details.
///
/// [`iter`]: struct.MapIndex.html#method.iter
/// [`iter_from`]: struct.MapIndex.html#method.iter_from
/// [`MapIndex`]: struct.MapIndex.html
#[derive(Debug)]
pub struct MapIndexIter<'a, K, V> {
    base_iter: BaseIndexIter<'a, K, V>,
}

/// Returns an iterator over the keys of a `MapIndex`.
///
/// This struct is created by the [`keys`] or
/// [`keys_from`] method on [`MapIndex`]. See its documentation for additional details.
///
/// [`keys`]: struct.MapIndex.html#method.keys
/// [`keys_from`]: struct.MapIndex.html#method.keys_from
/// [`MapIndex`]: struct.MapIndex.html
#[derive(Debug)]
pub struct MapIndexKeys<'a, K> {
    base_iter: BaseIndexIter<'a, K, ()>,
}

/// Returns an iterator over the values of a `MapIndex`.
///
/// This struct is created by the [`values`] or
/// [`values_from`] method on [`MapIndex`]. See its documentation for additional details.
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
    T: AsRef<dyn Snapshot>,
    K: BinaryKey,
    V: BinaryValue,
{
    /// Creates a new index representation based on the name and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case, only
    /// immutable methods are available. In the second case, both immutable and mutable methods are
    /// available.
    ///
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new(name, &snapshot);
    /// ```
    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {
        Self {
            base: BaseIndex::new(index_name, IndexType::Map, view),
            _k: PhantomData,
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
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let name = "name";
    /// let index_id = vec![01];
    ///
    /// let snapshot = db.snapshot();
    /// let index: MapIndex<_, u8, u8> = MapIndex::new_in_family(name, &index_id, &snapshot);
    /// ```
    pub fn new_in_family<S, I>(family_name: S, index_id: &I, view: T) -> Self
    where
        I: BinaryKey,
        I: ?Sized,
        S: AsRef<str>,
    {
        Self {
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
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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
        Q: BinaryKey + ?Sized,
    {
        self.base.get(key)
    }

    /// Returns `true` if the map contains a value corresponding to the specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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
        Q: BinaryKey + ?Sized,
    {
        self.base.contains(key)
    }

    /// Returns an iterator over the entries of the map in ascending order. The iterator element
    /// type is (K, V).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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

    /// Returns an iterator over the keys of a map in ascending order. The iterator element
    /// type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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

    /// Returns an iterator over the values of a map in ascending order of keys. The iterator
    /// element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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

    /// Returns an iterator over the entries of a map in ascending order starting from the
    /// specified key. The iterator element type is (K, V).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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
        Q: BinaryKey + ?Sized,
    {
        MapIndexIter {
            base_iter: self.base.iter_from(&(), from),
        }
    }

    /// Returns an iterator over the keys of a map in ascending order starting from the
    /// specified key. The iterator element type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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
        Q: BinaryKey + ?Sized,
    {
        MapIndexKeys {
            base_iter: self.base.iter_from(&(), from),
        }
    }

    /// Returns an iterator over the values of a map in ascending order of keys starting from the
    /// specified key. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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
        Q: BinaryKey + ?Sized,
    {
        MapIndexValues {
            base_iter: self.base.iter_from(&(), from),
        }
    }
}

impl<'a, K, V> MapIndex<&'a mut Fork, K, V>
where
    K: BinaryKey,
    V: BinaryValue,
{
    /// Inserts a key-value pair into a map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = MapIndex::new(name, &mut fork);
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    pub fn put(&mut self, key: &K, value: V) {
        self.base.put(key, value)
    }

    /// Removes a key from a map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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
        Q: BinaryKey + ?Sized,
    {
        self.base.remove(key)
    }

    /// Clears a map, removing all entries.
    ///
    /// # Notes
    /// Currently, this method is not optimized to delete a large set of data. During the execution of
    /// this method, the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
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
    T: AsRef<dyn Snapshot>,
    K: BinaryKey,
    V: BinaryValue,
{
    type Item = (K::Owned, V);
    type IntoIter = MapIndexIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> Iterator for MapIndexIter<'a, K, V>
where
    K: BinaryKey,
    V: BinaryValue,
{
    type Item = (K::Owned, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

impl<'a, K> Iterator for MapIndexKeys<'a, K>
where
    K: BinaryKey,
{
    type Item = K::Owned;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

impl<'a, V> Iterator for MapIndexValues<'a, V>
where
    V: BinaryValue,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(.., v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Database, TemporaryDB};
    use super::*;

    const IDX_NAME: &'static str = "idx_name";

    #[test]
    fn test_str_key() {
        let db = TemporaryDB::default();
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
    fn test_u8_slice_key() {
        let db = TemporaryDB::default();
        let mut fork = db.fork();

        const KEY: &[u8] = &[1, 2, 3];

        let mut index: MapIndex<_, Vec<u8>, _> = MapIndex::new(IDX_NAME, &mut fork);
        assert_eq!(false, index.contains(KEY));

        index.put(&KEY.to_owned(), 0);
        assert_eq!(true, index.contains(KEY));

        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    #[test]
    fn test_methods() {
        let db = TemporaryDB::default();
        let mut fork = db.fork();
        let mut map_index = MapIndex::new(IDX_NAME, &mut fork);

        assert_eq!(map_index.get(&1u8), None);
        assert!(!map_index.contains(&1u8));

        map_index.put(&1u8, 1u8);

        assert_eq!(map_index.get(&1u8), Some(1u8));
        assert!(map_index.contains(&1u8));

        map_index.remove(&1u8);

        assert!(!map_index.contains(&1u8));
        assert_eq!(map_index.get(&1u8), None);

        map_index.put(&2u8, 2u8);
        map_index.put(&3u8, 3u8);
        map_index.clear();

        assert!(!map_index.contains(&2u8));
        assert!(!map_index.contains(&3u8));
    }

    #[test]
    fn test_iter() {
        let db = TemporaryDB::default();
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
            map_index.iter_from(&0_u8).collect::<Vec<(u8, u8)>>(),
            vec![(2, 2), (3, 3)]
        );
        assert_eq!(
            map_index.iter_from(&1u8).collect::<Vec<(u8, u8)>>(),
            vec![(2, 2), (3, 3)]
        );
    }
}
