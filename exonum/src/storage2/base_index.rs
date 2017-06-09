use std::marker::PhantomData;

use super::{StorageKey, StorageValue, Snapshot, Fork, Iter};

pub struct BaseIndex<T> {
    prefix: Vec<u8>,
    view: T,
}

pub struct BaseIndexIter<'a, K, V> {
    base_iter: Iter<'a>,
    prefix: Vec<u8>,
    ended: bool,
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
        let mut v = vec![0; self.prefix.len() + K::size()];
        &mut v[..self.prefix.len()].copy_from_slice(&self.prefix);
        key.write(&mut v[self.prefix.len()..]);
        v
    }
}

impl<T> BaseIndex<T> where T: AsRef<Snapshot> {
    pub fn get<K, V>(&self, key: &K) -> Option<V> where K: StorageKey,
                                                        V: StorageValue {
        self.view.as_ref().get(&self.prefixed_key(key)).map(StorageValue::from_vec)
    }

    pub fn contains<K>(&self, key: &K) -> bool where K: StorageKey {
        self.view.as_ref().contains(&self.prefixed_key(key))
    }

    pub fn iter<P, K, V>(&self, prefix: &P) -> BaseIndexIter<K, V> where P: StorageKey,
                                                                         K: StorageKey,
                                                                         V: StorageValue {
        let iter_prefix = self.prefixed_key(prefix);
        BaseIndexIter {
            base_iter: self.view.as_ref().iter(&iter_prefix),
            prefix: iter_prefix,
            ended: false,
            _k: PhantomData,
            _v: PhantomData
        }
    }

    pub fn iter_from<P, F, K, V>(&self, prefix: &P, from: &F) -> BaseIndexIter<K, V> where P: StorageKey,
                                                                                           F: StorageKey,
                                                                                           K: StorageKey,
                                                                                           V: StorageValue {
        let iter_prefix = self.prefixed_key(prefix);
        let iter_from = self.prefixed_key(from)
        BaseIndexIter {
            base_iter: self.view.as_ref().iter(&iter_from),
            prefix: iter_prefix,
            ended: false,
            _k: PhantomData,
            _v: PhantomData
        }
    }
}

impl<T> BaseIndex<T> where T: AsMut<Fork> {
    pub fn put<K, V>(&mut self, key: &K, value: V) where K: StorageKey,
                                                         V: StorageValue {
        let key = self.prefixed_key(key);
        self.view.as_mut().put(key, value.into_vec());
    }

    pub fn delete<K>(&mut self, key: &K) where K: StorageKey {
        let key = self.prefixed_key(key);
        self.view.as_mut().delete(key);
    }

    pub fn clear(&mut self) {
        self.view.as_mut().delete_by_prefix(&self.prefix)
    }
}

impl<'a, K, V> Iterator for BaseIndexIter<'a, K, V> where K: StorageKey,
                                                          V: StorageValue, {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None
        }
        if let Some((ref k, ref v)) = self.base_iter.next() {
            if k.starts_with(&self.prefix) {
                return Some((K::read(&k[self.prefix.len()..]), V::from_slice(v)))
            }
        }
        self.ended = true;
        None
    }
}
