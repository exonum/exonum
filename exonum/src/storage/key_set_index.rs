//! An implementation of set for items that implements `StorageKey` trait.
use std::marker::PhantomData;

use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageKey};

/// A set of items that implemets `StorageKey` trait.
///
/// `KeySetIndex` implements a set, storing the element as keys with empty values.
/// `KeySetIndex` requires that the elements implement the [`StorageKey`] trait.
/// [`StorageKey`]: ../trait.StorageKey.html
#[derive(Debug)]
pub struct KeySetIndex<T, K> {
    base: BaseIndex<T>,
    _k: PhantomData<K>,
}

/// An iterator over the items of a `KeySetIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`KeySetIndex`]. See its documentation for more.
///
/// [`iter`]: struct.KeySetIndex.html#method.iter
/// [`iter_from`]: struct.KeySetIndex.html#method.iter_from
/// [`KeySetIndex`]: struct.KeySetIndex.html
#[derive(Debug)]
pub struct KeySetIndexIter<'a, K> {
    base_iter: BaseIndexIter<'a, K, ()>,
}

impl<T, K> KeySetIndex<T, K> {
    /// Creates a new index representation based on the common prefix of its keys and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    pub fn new(prefix: Vec<u8>, view: T) -> Self {
        KeySetIndex {
            base: BaseIndex::new(prefix, view),
            _k: PhantomData,
        }
    }
}

impl<T, K> KeySetIndex<T, K>
    where T: AsRef<Snapshot>,
          K: StorageKey
{
    /// Returns `true` if the set contains a value.
    pub fn contains(&self, item: &K) -> bool {
        self.base.contains(item)
    }

    /// An iterator visiting all elements in ascending order. The iterator element type is K.
    pub fn iter(&self) -> KeySetIndexIter<K> {
        KeySetIndexIter { base_iter: self.base.iter(&()) }
    }

    /// An iterator visiting all elements in arbitrary order starting from the specified value.
    /// The iterator element type is K.
    pub fn iter_from(&self, from: &K) -> KeySetIndexIter<K> {
        KeySetIndexIter { base_iter: self.base.iter_from(&(), from) }
    }
}

impl<'a, K> KeySetIndex<&'a mut Fork, K>
    where K: StorageKey
{
    /// Adds a value to the set.
    pub fn insert(&mut self, item: K) {
        self.base.put(&item, ())
    }

    /// Removes a value from the set.
    pub fn remove(&mut self, item: &K) {
        self.base.remove(item)
    }

    /// Clears the set, removing all values.
    ///
    /// # Notes
    /// Currently this method is not optimized to delete large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    pub fn clear(&mut self) {
        self.base.clear()
    }
}

impl<'a, T, K> ::std::iter::IntoIterator for &'a KeySetIndex<T, K>
    where T: AsRef<Snapshot>,
          K: StorageKey
{
    type Item = K;
    type IntoIter = KeySetIndexIter<'a, K>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K> Iterator for KeySetIndexIter<'a, K>
    where K: StorageKey
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}
