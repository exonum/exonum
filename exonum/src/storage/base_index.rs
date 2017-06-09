use std::borrow::Cow;
use std::marker::PhantomData;

use super::{StorageKey, StorageValue, Snapshot, Fork, Iter};

#[derive(Debug)]
pub struct BaseIndex<T> {
    prefix: Vec<u8>,
    view: T,
}

pub struct BaseIndexIter<'a, K, V> {
    base_iter: Iter<'a>,
    base_prefix_len: usize,
    prefix: Vec<u8>,
    ended: bool,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<T> BaseIndex<T> {
    pub fn new(prefix: Vec<u8>, view: T) -> Self {
        BaseIndex {
            prefix: prefix,
            view: view,
        }
    }

    fn prefixed_key<K: StorageKey>(&self, key: &K) -> Vec<u8> {
        let mut v = vec![0; self.prefix.len() + K::size()];
        &mut v[..self.prefix.len()].copy_from_slice(&self.prefix);
        key.write(&mut v[self.prefix.len()..]);
        v
    }
}

impl<T> BaseIndex<T>
    where T: AsRef<Snapshot>
{
    pub fn get<K, V>(&self, key: &K) -> Option<V>
        where K: StorageKey,
              V: StorageValue
    {
        self.view
            .as_ref()
            .get(&self.prefixed_key(key))
            .map(|v| StorageValue::from_bytes(Cow::Owned(v)))
    }

    pub fn contains<K>(&self, key: &K) -> bool
        where K: StorageKey
    {
        self.view.as_ref().contains(&self.prefixed_key(key))
    }

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
    pub fn put<K, V>(&mut self, key: &K, value: V)
        where K: StorageKey,
              V: StorageValue
    {
        let key = self.prefixed_key(key);
        self.view.put(key, value.into_vec());
    }

    pub fn remove<K>(&mut self, key: &K)
        where K: StorageKey
    {
        let key = self.prefixed_key(key);
        self.view.remove(key);
    }

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
        if let Some((ref k, ref v)) = self.base_iter.next() {
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
