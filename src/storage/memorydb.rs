use std::collections::BTreeMap;

use super::{Map, Database, Error, Patch, Change};

pub struct MemoryDB {
    map: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl MemoryDB {
    pub fn new() -> MemoryDB {
        MemoryDB { map: BTreeMap::new() }
    }
}

impl Map<[u8], Vec<u8>> for MemoryDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.map.get(key).map(Clone::clone))
    }

    fn put(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
        self.map.insert(key.to_vec(), value);
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), Error> {
        self.map.remove(key);
        Ok(())
    }
}

impl Database for MemoryDB {
    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        for (key, change) in patch.changes.into_iter() {
            match change {
                Change::Put(ref v) => {
                    self.map.insert(key.clone(), v.clone());
                }
                Change::Delete => {
                    self.map.remove(&key);
                }
            }
        }
        Ok(())
    }
}