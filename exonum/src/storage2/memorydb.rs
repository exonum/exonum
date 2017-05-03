use std::clone::Clone;
use std::collections::BTreeMap;

use super::{Map, Database, Snapshot, Result, Patch, Change, Fork};

#[derive(Default, Clone)]
pub struct MemoryDB {
    map: BTreeMap<Vec<u8>, Vec<u8>>
}

impl MemoryDB {
    pub fn new() -> MemoryDB {
        MemoryDB { map: BTreeMap::new() }
    }
}

impl Snapshot for MemoryDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.map.get(key).map(Clone::clone))
    }
}

impl Database for MemoryDB {
    fn snapshot(&self) -> Box<Snapshot> {
        Box::new(self.clone())
    }

    fn merge(&mut self, patch: Patch) -> Result<()> {
        for (key, change) in patch {
            match change {
                Change::Put(value) => { self.map.insert(key, value); },
                Change::Delete => { self.map.remove(&key); }
            }
        }
        Ok(())
    }
}
