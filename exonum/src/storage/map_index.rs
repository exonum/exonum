use std::marker::PhantomData;

use super::{BaseIndex, BaseIndexIter, Snapshot, Fork, StorageKey, StorageValue};

#[derive(Debug)]
pub struct MapIndex<T, K, V> {
    base: BaseIndex<T>,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

#[derive(Debug)]
pub struct MapIndexIter<'a, K, V> {
    base_iter: BaseIndexIter<'a, K, V>
}

#[derive(Debug)]
pub struct MapIndexKeys<'a, K> {
    base_iter: BaseIndexIter<'a, K, ()>
}

#[derive(Debug)]
pub struct MapIndexValues<'a, V> {
    base_iter: BaseIndexIter<'a, (), V>
}

impl<T, K, V> MapIndex<T, K, V> {
    pub fn new(prefix: Vec<u8>, base: T) -> Self {
        MapIndex {
            base: BaseIndex::new(prefix, base),
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<T, K, V> MapIndex<T, K, V> where T: AsRef<Snapshot>,
                                      K: StorageKey,
                                      V: StorageValue {
    pub fn get(&self, key: &K) -> Option<V> {
        self.base.get(key)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.base.contains(key)
    }

    pub fn iter(&self) -> MapIndexIter<K, V> {
        MapIndexIter { base_iter: self.base.iter(&()) }
    }

    pub fn keys(&self) -> MapIndexKeys<K> {
        MapIndexKeys { base_iter: self.base.iter(&()) }
    }

    pub fn values(&self) -> MapIndexValues<V> {
        MapIndexValues { base_iter: self.base.iter(&()) }
    }

    pub fn iter_from(&self, from: &K) -> MapIndexIter<K, V> {
        MapIndexIter { base_iter: self.base.iter_from(&(), from) }
    }

    pub fn keys_from(&self, from: &K) -> MapIndexKeys<K> {
        MapIndexKeys { base_iter: self.base.iter_from(&(), from) }
    }

    pub fn values_from(&self, from: &K) -> MapIndexValues<V> {
        MapIndexValues { base_iter: self.base.iter_from(&(), from) }
    }
}

impl<'a, K, V> MapIndex<&'a mut Fork, K, V> where K: StorageKey,
                                                  V: StorageValue {
    pub fn put(&mut self, key: &K, value: V) {
        self.base.put(key, value)
    }

    pub fn remove(&mut self, key: &K) {
        self.base.remove(key)
    }

    pub fn clear(&mut self) {
        self.base.clear()
    }
}

impl<'a, T, K, V> ::std::iter::IntoIterator for &'a MapIndex<T, K, V> where T: AsRef<Snapshot>,
                                                                            K: StorageKey,
                                                                            V: StorageValue {
    type Item = (K, V);
    type IntoIter = MapIndexIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> Iterator for MapIndexIter<'a, K, V> where K: StorageKey,
                                                         V: StorageValue, {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next()
    }
}

impl<'a, K> Iterator for MapIndexKeys<'a, K> where K: StorageKey {
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

impl<'a, V> Iterator for MapIndexValues<'a, V> where V: StorageValue {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(.., v)| v)
    }
}

