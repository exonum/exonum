use super::{Result, StorageKey, StorageValue, Snapshot, Fork};

pub struct BaseIndex<T> {
    prefix: Vec<u8>,
    view: T,
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
        Ok(self.view.as_ref().get(&self.prefixed_key(key))?.map(StorageValue::deserialize))
    }

    pub fn contains<K>(&self, key: &K) -> Result<bool> where K: StorageKey {
        self.view.as_ref().contains(&self.prefixed_key(key))
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
