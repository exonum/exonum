use std::marker::PhantomData;

use super::{BaseIndex, Error, Snapshot, Fork, StorageKey, StorageValue};

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
    pub fn get(&self, key: &K) -> Result<Option<V>, Error> {
        self.base.get(key)
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
