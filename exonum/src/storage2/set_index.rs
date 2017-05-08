use std::marker::PhantomData;

use super::{BaseIndex, BaseIndexIter, Result, Snapshot, Fork, StorageKey, StorageValue};

pub struct SetIndex<T, K> {
    base: BaseIndex<T>,
    _k: PhantomData<K>,
}

pub struct SetIndexIter<'a, K> {
    base_iter: BaseIndexIter<'a, K, ()>
}

impl<T, K> SetIndex<T, K> {
    pub fn new(prefix: Vec<u8>, base: T) -> Self {
        SetIndex {
            base: BaseIndex::new(prefix, base),
            _k: PhantomData,
        }
    }
}

impl<T, K> SetIndex<T, K> where T: AsRef<Snapshot>,
                                K: StorageKey {
    pub fn contains(&self, key: &K) -> Result<bool> {
        self.base.contains(key)
    }

    pub fn iter(&self) -> SetIndexIter<K> {
        SetIndexIter { base_iter: self.base.iter() }
    }

    pub fn iter_from(&self, from: &K) -> SetIndexIter<K> {
        SetIndexIter { base_iter: self.base.iter_from(from) }
    }
}

impl<T, K> SetIndex<T, K> where T: AsMut<Fork>,
                                K: StorageKey {
    pub fn insert(&mut self, key: &K) {
        self.base.put(key, ())
    }

    pub fn delete(&mut self, key: &K) {
        self.base.delete(key)
    }
}

impl<'a, K> Iterator for SetIndexIter<'a, K> where K: StorageKey {
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}
