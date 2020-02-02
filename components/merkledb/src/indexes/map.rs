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

//! An implementation of a key-value map.
//!
//! `MapIndex` requires that keys implement the [`BinaryKey`] trait and values implement
//! the [`BinaryValue`] trait. The given section contains methods related to
//! `MapIndex` and iterators over the items of this map.

use std::{borrow::Borrow, marker::PhantomData};

use crate::{
    access::{Access, AccessError, FromAccess},
    indexes::iter::{Entries, IndexIterator, Keys, Values},
    views::{IndexAddress, IndexType, RawAccess, RawAccessMut, View, ViewWithMetadata},
    BinaryKey, BinaryValue,
};

/// A map of keys and values. Access to the elements of this map is obtained using the keys.
///
/// `MapIndex` requires that keys implement the [`BinaryKey`] trait and values implement
/// the [`BinaryValue`] trait.
///
/// [`BinaryKey`]: ../trait.BinaryKey.html
/// [`BinaryValue`]: ../trait.BinaryValue.html
#[derive(Debug)]
pub struct MapIndex<T: RawAccess, K: ?Sized, V> {
    base: View<T>,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<T, K, V> FromAccess<T> for MapIndex<T::Base, K, V>
where
    T: Access,
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    fn from_access(access: T, addr: IndexAddress) -> Result<Self, AccessError> {
        let view = access.get_or_create_view(addr, IndexType::Map)?;
        Ok(Self::new(view))
    }
}

impl<T, K, V> MapIndex<T, K, V>
where
    T: RawAccess,
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    fn new(view: ViewWithMetadata<T>) -> Self {
        let base = view.into();
        Self {
            base,
            _v: PhantomData,
            _k: PhantomData,
        }
    }

    /// Returns a value corresponding to the key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let mut index = fork.get_map("name");
    /// assert!(index.get(&1).is_none());
    ///
    /// index.put(&1, 2);
    /// assert_eq!(Some(2), index.get(&1));
    /// ```
    pub fn get(&self, key: &K) -> Option<V> {
        self.base.get(key)
    }

    /// Returns `true` if the map contains a value corresponding to the specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let mut index = fork.get_map("name");
    /// assert!(!index.contains(&1));
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    /// ```
    pub fn contains(&self, key: &K) -> bool {
        self.base.contains(key)
    }

    /// Returns an iterator over the entries of the map in ascending order.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let index: MapIndex<_, u8, u8> = fork.get_map("name");
    ///
    /// for v in index.iter() {
    ///     println!("{:?}", v);
    /// }
    /// ```
    pub fn iter(&self) -> Entries<'_, K, V> {
        self.index_iter(None)
    }

    /// Returns an iterator over the keys of a map in ascending order.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let index: MapIndex<_, u8, u8> = fork.get_map("name");
    ///
    /// for key in index.keys() {
    ///     println!("{}", key);
    /// }
    /// ```
    pub fn keys(&self) -> Keys<'_, K> {
        self.iter().skip_values()
    }

    /// Returns an iterator over the values of a map in ascending order of keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let index: MapIndex<_, u8, u8> = fork.get_map("name");
    ///
    /// for val in index.values() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values(&self) -> Values<'_, V> {
        self.iter().skip_keys()
    }

    /// Returns an iterator over the entries of a map in ascending order starting from the
    /// specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let index: MapIndex<_, u8, u8> = fork.get_map("name");
    ///
    /// for v in index.iter_from(&2) {
    ///     println!("{:?}", v);
    /// }
    /// ```
    pub fn iter_from(&self, from: &K) -> Entries<'_, K, V> {
        self.index_iter(Some(from))
    }

    /// Returns an iterator over the keys of a map in ascending order starting from the
    /// specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let index: MapIndex<_, u8, u8> = fork.get_map("name");
    ///
    /// for key in index.keys_from(&2) {
    ///     println!("{}", key);
    /// }
    /// ```
    pub fn keys_from(&self, from: &K) -> Keys<'_, K> {
        self.iter_from(from).skip_values()
    }

    /// Returns an iterator over the values of a map in ascending order of keys starting from the
    /// specified key.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let index: MapIndex<_, u8, u8> = fork.get_map("name");
    /// for val in index.values_from(&2) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values_from(&self, from: &K) -> Values<'_, V> {
        self.iter_from(from).skip_keys()
    }
}

impl<T, K, V> MapIndex<T, K, V>
where
    T: RawAccessMut,
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    /// Inserts a key-value pair into a map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let mut index = fork.get_map("name");
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    /// ```
    pub fn put(&mut self, key: &K, value: V) {
        self.base.put(key, value);
    }

    /// Removes a key from a map.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let mut index = fork.get_map("name");
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    ///
    /// index.remove(&1);
    /// assert!(!index.contains(&1));
    /// ```
    pub fn remove<Q>(&mut self, key: &Q)
    where
        K: Borrow<Q>,
        Q: BinaryKey + ?Sized,
    {
        self.base.remove(key);
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
    /// use exonum_merkledb::{access::CopyAccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let mut index = fork.get_map("name");
    ///
    /// index.put(&1, 2);
    /// assert!(index.contains(&1));
    ///
    /// index.clear();
    /// assert!(!index.contains(&1));
    /// ```
    pub fn clear(&mut self) {
        self.base.clear();
    }
}

impl<'a, T, K, V> IntoIterator for &'a MapIndex<T, K, V>
where
    T: RawAccess,
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    type Item = (K::Owned, V);
    type IntoIter = Entries<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T, K, V> IndexIterator for MapIndex<T, K, V>
where
    T: RawAccess,
    K: BinaryKey + ?Sized,
    V: BinaryValue,
{
    type Key = K;
    type Value = V;

    fn index_iter(&self, from: Option<&K>) -> Entries<'_, K, V> {
        Entries::new(&self.base, from)
    }
}

#[cfg(test)]
mod tests {
    use crate::{access::CopyAccessExt, Database, TemporaryDB};

    const IDX_NAME: &str = "idx_name";

    #[test]
    fn test_str_key() {
        const KEY: &str = "key_1";
        let db = TemporaryDB::default();
        let fork = db.fork();

        let mut index = fork.get_map(IDX_NAME);
        assert_eq!(false, index.contains(KEY));
        index.put(&KEY, 0);
        assert_eq!(true, index.contains(KEY));
        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    #[test]
    fn test_u8_slice_key() {
        const KEY: &[u8] = &[1, 2, 3];
        let db = TemporaryDB::default();
        let fork = db.fork();

        let mut index = fork.get_map(IDX_NAME);
        assert_eq!(false, index.contains(KEY));

        index.put(&KEY, 0);
        assert_eq!(true, index.contains(KEY));

        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }

    #[test]
    fn test_methods() {
        let db = TemporaryDB::default();
        let fork = db.fork();

        let mut map_index = fork.get_map(IDX_NAME);
        assert_eq!(map_index.get(&1_u8), None);
        assert!(!map_index.contains(&1_u8));

        map_index.put(&1_u8, 1_u8);
        assert_eq!(map_index.get(&1_u8), Some(1_u8));
        assert!(map_index.contains(&1_u8));

        map_index.remove(&100_u8);
        map_index.remove(&1_u8);
        assert!(!map_index.contains(&1_u8));
        assert_eq!(map_index.get(&1_u8), None);

        map_index.put(&2_u8, 2_u8);
        map_index.put(&3_u8, 3_u8);
        map_index.clear();
        assert!(!map_index.contains(&2_u8));
        assert!(!map_index.contains(&3_u8));
    }

    #[test]
    fn test_iter() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut map_index = fork.get_map(IDX_NAME);

        map_index.put(&1_u8, 1_u8);
        map_index.put(&2_u8, 2_u8);
        map_index.put(&3_u8, 3_u8);

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

        map_index.remove(&1_u8);
        assert_eq!(
            map_index.iter_from(&0_u8).collect::<Vec<(u8, u8)>>(),
            vec![(2, 2), (3, 3)]
        );
        assert_eq!(
            map_index.iter_from(&1_u8).collect::<Vec<(u8, u8)>>(),
            vec![(2, 2), (3, 3)]
        );
    }

    #[test]
    fn index_as_iterator() {
        let db = TemporaryDB::default();
        let fork = db.fork();
        let mut map_index = fork.get_map(IDX_NAME);

        map_index.put(&1_u8, 1_u8);
        map_index.put(&2_u8, 2_u8);
        map_index.put(&3_u8, 3_u8);

        for (key, value) in &map_index {
            assert!(key == value);
        }
        assert_eq!((&map_index).into_iter().count(), 3);
        assert_eq!(map_index.keys().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(
            map_index.iter().collect::<Vec<_>>(),
            vec![(1, 1), (2, 2), (3, 3)]
        );

        let mut map_index = fork.get_map((IDX_NAME, &0_u8));
        map_index.put("1", 1_u8);
        map_index.put("2", 2_u8);
        map_index.put("3", 3_u8);
        for (key, value) in &map_index {
            assert_eq!(key, value.to_string());
        }
        assert_eq!((&map_index).into_iter().count(), 3);
        assert_eq!(map_index.keys().collect::<Vec<_>>(), vec!["1", "2", "3"]);
        assert_eq!(
            map_index.iter().collect::<Vec<_>>(),
            vec![
                ("1".to_owned(), 1),
                ("2".to_owned(), 2),
                ("3".to_owned(), 3)
            ]
        );
    }
}
