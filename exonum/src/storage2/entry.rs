use std::marker::PhantomData;

use crypto::Hash;

use super::{BaseIndex, Result, Snapshot, Fork, StorageValue};

pub struct Entry<T, V> {
    base: BaseIndex<T>,
    _v: PhantomData<V>,
}

impl<T, V> Entry<T, V> {
    pub fn new(prefix: Vec<u8>, base: T) -> Self {
        Entry {
            base: BaseIndex::new(prefix, base),
            _v: PhantomData,
        }
    }
}

impl<T, V> Entry<T, V> where T: AsRef<Snapshot>,
                             V: StorageValue {
    pub fn get(&self) -> Result<Option<V>> {
        self.base.get(&())
    }

    pub fn exists(&self) -> Result<bool> {
        self.base.contains(&())
    }

    pub fn hash(&self) -> Result<Hash> {
        Ok(self.base.get::<(), V>(&())?.map(|v| v.hash()).unwrap_or_default())
    }
}

impl<'a, V> Entry<&'a mut Fork, V> where V: StorageValue {
    pub fn put(&mut self, value: V) {
        self.base.put(&(), value)
    }

    pub fn delete(&mut self) {
        self.base.delete(&())
    }
}
