use super::{Map, View, Error, StorageKey};

pub struct BaseTable<'a> {
    prefix: Vec<u8>,
    view: &'a View,
}

impl<'a> BaseTable<'a> {
    pub fn new(prefix: Vec<u8>, view: &'a View) -> Self {
        BaseTable {
            prefix: prefix,
            view: view,            
        }
    }

    fn prefixed_key<K: StorageKey>(&self, key: &K) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.prefix.len() + K::size());
        v.extend_from_slice(&self.prefix);
        key.write(&mut v);
        v
    }

    fn get<K: StorageKey>(&self, key: &K) -> Result<Option<Vec<u8>>, Error> {        
        self.view.get(&self.prefixed_key(key))
    }

    fn put<K: StorageKey>(&self, key: &K, value: Vec<u8>) -> Result<(), Error> {
        self.view.put(&self.prefixed_key(key), value)
    }

    fn delete<K: StorageKey>(&self, key: &K) -> Result<(), Error> {
        self.view.delete(&self.prefixed_key(key))
    }
}