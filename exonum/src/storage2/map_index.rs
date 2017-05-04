use std::marker::PhantomData;

use super::{BaseIndex, Result, Snapshot, Fork, StorageKey, StorageValue};

pub struct MapIndex<T, K, V> {
    base: BaseIndex<T>,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<T, K, V> MapIndex<T, K, V> {
    pub fn new(prefix: Vec<u8>, view: T) -> Self {
        MapIndex {
            base: BaseIndex::new(prefix, view),
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<T, K, V> MapIndex<T, K, V> where T: AsRef<Snapshot>,
                                      K: StorageKey,
                                      V: StorageValue {
    pub fn get(&self, key: &K) -> Result<Option<V>> {
        self.base.get(key)
    }

    pub fn contains(&self, key: &K) -> Result<bool> {
        self.base.contains(key)
    }
}

impl<T, K, V> MapIndex<T, K, V> where T: AsMut<Fork>,
                                      K: StorageKey,
                                      V: StorageValue {
    pub fn put(&mut self, key: &K, value: V) {
        self.base.put(key, value)
    }

    pub fn delete(&mut self, key: &K) {
        self.base.delete(key)
    }
}
