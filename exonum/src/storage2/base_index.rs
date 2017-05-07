use std::marker::PhantomData;

use super::{Result, StorageKey, StorageValue, Snapshot, Fork, Iter};

pub struct BaseIndex<T> {
    prefix: Vec<u8>,
    view: T,
}

pub struct BaseIndexIter<'a, K, V> {
    prefix: &'a [u8],
    base_iter: Iter<'a>,
    stopped: bool,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<T> BaseIndex<T> {
    pub fn new(prefix: Vec<u8>, view: T) -> Self {
        BaseIndex {
            prefix: prefix,
            view: view
        }
    }

    fn prefixed_key<K: StorageKey>(&self, key: &K) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.prefix.len() + K::size());
        v.extend_from_slice(&self.prefix);
        key.write(&mut v);
        v
    }
}

impl<T> BaseIndex<T> where T: AsRef<Snapshot> {
    pub fn get<K, V>(&self, key: &K) -> Result<Option<V>> where K: StorageKey,
                                                                V: StorageValue {
        Ok(self.view.as_ref().get(&self.prefixed_key(key))?.map(StorageValue::from_vec))
    }

    pub fn contains<K>(&self, key: &K) -> Result<bool> where K: StorageKey {
        self.view.as_ref().contains(&self.prefixed_key(key))
    }

    pub fn iter<K, V>(&self) -> BaseIndexIter<K, V> where K: StorageKey,
                                                          V: StorageValue {
        BaseIndexIter {
            prefix: &self.prefix,
            base_iter: self.view.as_ref().iter(&self.prefix),
            stopped: false,
            _k: PhantomData,
            _v: PhantomData
        }
    }
}

impl<T> BaseIndex<T> where T: AsMut<Fork> {
    pub fn put<K, V>(&mut self, key: &K, value: V) where K: StorageKey,
                                                         V: StorageValue {
        let key = self.prefixed_key(key);
        self.view.as_mut().put(key, value.serialize());
    }

    pub fn delete<K>(&mut self, key: &K) where K: StorageKey {
        let key = self.prefixed_key(key);
        self.view.as_mut().delete(key);
    }
}

impl<'a, K, V> Iterator for BaseIndexIter<'a, K, V> where K: StorageKey,
                                                          V: StorageValue, {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None
        }
        if let Some((ref k, ref v)) = self.base_iter.next() {
            if k.starts_with(self.prefix) {
                return Some((K::read(k), V::from_slice(v)))
            }
        }
        self.stopped = true;
        None
    }
}
