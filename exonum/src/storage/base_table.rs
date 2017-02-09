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

    pub fn prefixed_key<K: StorageKey>(&self, key: &K) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.prefix.len() + key.size());
        v.extend_from_slice(&self.prefix);
        key.write(&mut v);
        v
    }

    pub fn get<K: StorageKey>(&self, key: &K) -> Result<Option<Vec<u8>>, Error> {        
        self.view.get(&self.prefixed_key(key))
    }

    pub fn put<K: StorageKey>(&self, key: &K, value: Vec<u8>) -> Result<(), Error> {
        self.view.put(&self.prefixed_key(key), value)
    }

    pub fn delete<K: StorageKey>(&self, key: &K) -> Result<(), Error> {
        self.view.delete(&self.prefixed_key(key))
    }

    // FIXME: remove this
    pub fn find_key(&self, _: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        unimplemented!();
    }
}