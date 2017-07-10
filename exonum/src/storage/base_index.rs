//! Implementation of base index with most common features.
use std::borrow::Cow;
use std::marker::PhantomData;

use super::{StorageKey, StorageValue, Snapshot, Fork, Iter};

/// Basic struct for all indices that implements common features.
///
/// `BaseIndex` requires that the keys implement the [`StorageKey`] trait and the values implement
/// [`StorageValue`] trait. However, this structure is not bound to specific types and allows the
/// use of *any* types as keys or values.
/// [`StorageKey`]: ../trait.StorageKey.html
/// [`StorageValue`]: ../trait.StorageValue.html
#[derive(Debug)]
pub struct BaseIndex<T> {
    prefix: Vec<u8>,
    view: T,
}

/// An iterator over an entries of a `BaseIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] methods on [`BaseIndex`]. See its documentation for more.
///
/// [`iter`]: struct.BaseIndex.html#method.iter
/// [`iter_from`]: struct.BaseIndex.html#method.iter_from
/// [`BaseIndex`]: struct.BaseIndex.html
pub struct BaseIndexIter<'a, K, V> {
    base_iter: Iter<'a>,
    base_prefix_len: usize,
    prefix: Vec<u8>,
    ended: bool,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<T> BaseIndex<T> {
    /// Creates a new index representation based on the common prefix of its keys and storage view.
    ///
    /// Storage view can be specified as [`&Snapshot`] or [`&mut Fork`]. In the first case only
    /// immutable methods are available. In the second case both immutable and mutable methods are
    /// available.
    /// [`&Snapshot`]: ../trait.Snapshot.html
    /// [`&mut Fork`]: ../struct.Fork.html
    pub fn new(prefix: Vec<u8>, view: T) -> Self {
        BaseIndex {
            prefix: prefix,
            view: view,
        }
    }

    fn prefixed_key<K: StorageKey>(&self, key: &K) -> Vec<u8> {
        let mut v = vec![0; self.prefix.len() + key.size()];
        v[..self.prefix.len()].copy_from_slice(&self.prefix);
        key.write(&mut v[self.prefix.len()..]);
        v
    }
}

impl<T> BaseIndex<T>
    where T: AsRef<Snapshot>
{
    /// Returns a value of *any* corresponding to the key of *any* type.
    pub fn get<K, V>(&self, key: &K) -> Option<V>
        where K: StorageKey,
              V: StorageValue
    {
        self.view
            .as_ref()
            .get(&self.prefixed_key(key))
            .map(|v| StorageValue::from_bytes(Cow::Owned(v)))
    }

    /// Returns `true` if the index contains a value for the specified key of *any* type.
    pub fn contains<K>(&self, key: &K) -> bool
        where K: StorageKey
    {
        self.view.as_ref().contains(&self.prefixed_key(key))
    }

    /// Returns an iterator over the entries of the index in ascending order. The iterator element
    /// type is any key-value pair. An argument `subprefix` allows to specify a subset of
    /// iteration.
    pub fn iter<P, K, V>(&self, subprefix: &P) -> BaseIndexIter<K, V>
        where P: StorageKey,
              K: StorageKey,
              V: StorageValue
    {
        let iter_prefix = self.prefixed_key(subprefix);
        BaseIndexIter {
            base_iter: self.view.as_ref().iter(&iter_prefix),
            base_prefix_len: self.prefix.len(),
            prefix: iter_prefix,
            ended: false,
            _k: PhantomData,
            _v: PhantomData,
        }
    }

    /// Returns an iterator over the entries of the index in ascending order starting from the
    /// specified key. The iterator element type is any key-value pair. An argument `subprefix`
    /// allows to specify a subset of iteration.
    pub fn iter_from<P, F, K, V>(&self, subprefix: &P, from: &F) -> BaseIndexIter<K, V>
        where P: StorageKey,
              F: StorageKey,
              K: StorageKey,
              V: StorageValue
    {
        let iter_prefix = self.prefixed_key(subprefix);
        let iter_from = self.prefixed_key(from);
        BaseIndexIter {
            base_iter: self.view.as_ref().iter(&iter_from),
            base_prefix_len: self.prefix.len(),
            prefix: iter_prefix,
            ended: false,
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<'a> BaseIndex<&'a mut Fork> {
    /// Inserts the key-value pair into the index. Both key and value may be of *any* types.
    pub fn put<K, V>(&mut self, key: &K, value: V)
        where K: StorageKey,
              V: StorageValue
    {
        let key = self.prefixed_key(key);
        self.view.put(key, value.into_bytes());
    }

    /// Removes the key of *any* type from the index.
    pub fn remove<K>(&mut self, key: &K)
        where K: StorageKey
    {
        let key = self.prefixed_key(key);
        self.view.remove(key);
    }

    /// Clears the index, removing all entries.
    ///
    /// # Notes
    /// Currently this method is not optimized to delete large set of data. During the execution of
    /// this method the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    pub fn clear(&mut self) {
        self.view.remove_by_prefix(&self.prefix)
    }
}

impl<'a, K, V> Iterator for BaseIndexIter<'a, K, V>
    where K: StorageKey,
          V: StorageValue
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        if let Some((k, v)) = self.base_iter.next() {
            if k.starts_with(&self.prefix) {
                return Some((K::read(&k[self.base_prefix_len..]), V::from_bytes(Cow::Borrowed(v))));
            }
        }
        self.ended = true;
        None
    }
}

impl<'a, K, V> ::std::fmt::Debug for BaseIndexIter<'a, K, V> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "BaseIndexIter(..)")
    }
}
