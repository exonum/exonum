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

use std::{borrow::Borrow, fmt, marker::PhantomData};

use crate::{
    access::{Access, AccessError, FromAccess},
    views::{
        IndexAddress, IndexType, Iter as ViewIter, RawAccess, RawAccessMut, View, ViewWithMetadata,
    },
    BinaryKey, BinaryValue,
};

/// A map of keys and values. Access to the elements of this map is obtained using the keys.
///
/// `MapIndex` requires that keys implement the [`BinaryKey`] trait and values implement
/// the [`BinaryValue`] trait.
///
/// [`BinaryKey`]: trait.BinaryKey.html
/// [`BinaryValue`]: trait.BinaryValue.html
#[derive(Debug)]
pub struct MapIndex<T: RawAccess, K: ?Sized, V> {
    base: View<T>,
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
pub struct Iter<'a, K: ?Sized, V> {
    base_iter: ViewIter<'a, K, V>,
}

/// Returns an iterator over the keys of a `MapIndex`.
///
/// This struct is created by the [`keys`] or
/// [`keys_from`] method on [`MapIndex`]. See its documentation for additional details.
///
/// [`keys`]: struct.MapIndex.html#method.keys
/// [`keys_from`]: struct.MapIndex.html#method.keys_from
/// [`MapIndex`]: struct.MapIndex.html
pub struct Keys<'a, K: ?Sized> {
    base_iter: ViewIter<'a, K, ()>,
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
pub struct Values<'a, V> {
    base_iter: ViewIter<'a, (), V>,
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
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
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
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
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

    /// Returns an iterator over the entries of the map in ascending order. The iterator element
    /// type is (K, V).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let index: MapIndex<_, u8, u8> = fork.get_map("name");
    ///
    /// for v in index.iter() {
    ///     println!("{:?}", v);
    /// }
    /// ```
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            base_iter: self.base.iter(&()),
        }
    }

    /// Returns an iterator over the keys of a map in ascending order. The iterator element
    /// type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
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
        Keys {
            base_iter: self.base.iter(&()),
        }
    }

    /// Returns an iterator over the values of a map in ascending order of keys. The iterator
    /// element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
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
        Values {
            base_iter: self.base.iter(&()),
        }
    }

    /// Returns an iterator over the entries of a map in ascending order starting from the
    /// specified key. The iterator element type is (K, V).
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let index: MapIndex<_, u8, u8> = fork.get_map("name");
    ///
    /// for v in index.iter_from(&2) {
    ///     println!("{:?}", v);
    /// }
    /// ```
    pub fn iter_from(&self, from: &K) -> Iter<'_, K, V> {
        Iter {
            base_iter: self.base.iter_from(&(), from),
        }
    }

    /// Returns an iterator over the keys of a map in ascending order starting from the
    /// specified key. The iterator element type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
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
        Keys {
            base_iter: self.base.iter_from(&(), from),
        }
    }

    /// Returns an iterator over the values of a map in ascending order of keys starting from the
    /// specified key. The iterator element type is V.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
    ///
    /// let db = TemporaryDB::default();
    /// let fork = db.fork();
    /// let index: MapIndex<_, u8, u8> = fork.get_map("name");
    /// for val in index.values_from(&2) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn values_from(&self, from: &K) -> Values<'_, V> {
        Values {
            base_iter: self.base.iter_from(&(), from),
        }
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
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
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
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
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
    /// use exonum_merkledb::{access::AccessExt, TemporaryDB, Database, MapIndex};
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

impl<'a, T, K, V> std::iter::IntoIterator for &'a MapIndex<T, K, V>
where
    T: RawAccess,
    K: BinaryKey,
    V: BinaryValue,
{
    type Item = (K::Owned, V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: BinaryKey,
    V: BinaryValue,
{
    type Item = (K::Owned, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

impl<'a, K> Iterator for Keys<'a, K>
where
    K: BinaryKey,
{
    type Item = K::Owned;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

impl<'a, V> Iterator for Values<'a, V>
where
    V: BinaryValue,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(.., v)| v)
    }
}

impl<'a, K, V> fmt::Debug for Iter<'a, K, V>
where
    K: BinaryKey,
    V: BinaryValue,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("MapIndexIter").finish()
    }
}

impl<'a, K> fmt::Debug for Keys<'a, K>
where
    K: BinaryKey,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("MapIndexKeys").finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::{access::AccessExt, Database, TemporaryDB};

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
}
