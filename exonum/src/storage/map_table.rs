use std::marker::PhantomData;

use profiler;

use super::{Map, Error, StorageValue};

pub struct MapTable<'a, T: Map<[u8], Vec<u8>> + 'a, K: ?Sized, V> {
    prefix: Vec<u8>,
    storage: &'a T,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<'a, T: Map<[u8], Vec<u8>> + 'a, K: ?Sized, V> MapTable<'a, T, K, V> {
    pub fn new(prefix: Vec<u8>, storage: &'a T) -> Self {
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
        let _profiler = profiler::ProfilerSpan::new("MapTable::get");
        let v = self.storage.get(&[&self.prefix, key.as_ref()].concat())?;
        Ok(v.map(StorageValue::deserialize))
    }

    fn put(&self, key: &K, value: V) -> Result<(), Error> {
        self.storage.put(&[&self.prefix, key.as_ref()].concat(), value.serialize())
    }

    fn delete(&self, key: &K) -> Result<(), Error> {
        self.storage.delete(&[&self.prefix, key.as_ref()].concat())
    }
    fn find_key(&self, origin_key: &K) -> Result<Option<Vec<u8>>, Error> {
        let key = [&self.prefix, origin_key.as_ref()].concat();
        let result = match self.storage.find_key(&key)? {
            Some(x) => {
                if !x.starts_with(&key) {
                    None
                } else {
                    Some(x[self.prefix.len()..].to_vec())
                }
            }
            None => None,
        };
        Ok(result)
    }
}
