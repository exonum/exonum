use std::marker::PhantomData;

use super::{Map, Error, StorageValue};

pub struct MapTable<'a, T: Map<[u8], Vec<u8>> + 'a, K: ?Sized, V> {
    prefix: Vec<u8>,
    storage: &'a mut T,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<'a, T: Map<[u8], Vec<u8>> + 'a, K: ?Sized, V> MapTable<'a, T, K, V> {
    pub fn new(prefix: Vec<u8>, storage: &'a mut T) -> Self {
        MapTable {
            prefix: prefix,
            storage: storage,
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

impl<'a, T, K: ?Sized, V> Map<K, V> for MapTable<'a, T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: AsRef<[u8]>,
          V: StorageValue
{
    fn get(&self, key: &K) -> Result<Option<V>, Error> {
        let v = self.storage.get(&[&self.prefix, key.as_ref()].concat())?;
        Ok(v.map(StorageValue::deserialize))
    }

    fn put(&mut self, key: &K, value: V) -> Result<(), Error> {
        self.storage.put(&[&self.prefix, key.as_ref()].concat(), value.serialize())
    }

    fn delete(&mut self, key: &K) -> Result<(), Error> {
        self.storage.delete(&[&self.prefix, key.as_ref()].concat())
    }
}
