use std::marker::PhantomData;

use crypto::Hash;

use super::{BaseIndex, Snapshot, Fork, StorageValue};

#[derive(Debug)]
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

impl<T, V> Entry<T, V>
    where T: AsRef<Snapshot>,
          V: StorageValue
{
    pub fn get(&self) -> Option<V> {
        self.base.get(&())
    }

    pub fn exists(&self) -> bool {
        self.base.contains(&())
    }

    pub fn hash(&self) -> Hash {
        self.base
            .get::<(), V>(&())
            .map(|v| v.hash())
            .unwrap_or_default()
    }
}

impl<'a, V> Entry<&'a mut Fork, V>
    where V: StorageValue
{
    pub fn set(&mut self, value: V) {
        self.base.put(&(), value)
    }

    pub fn remove(&mut self) {
        self.base.remove(&())
    }
}
