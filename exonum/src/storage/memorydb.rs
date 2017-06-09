use std::clone::Clone;
use std::collections::BTreeMap;

use super::{Database, Snapshot, Patch, Change, Iter, Result};

#[derive(Default, Clone)]
pub struct MemoryDB {
    map: BTreeMap<Vec<u8>, Vec<u8>>
}

impl MemoryDB {
    pub fn new() -> MemoryDB {
        MemoryDB { map: BTreeMap::new() }
    }
}

impl Database for MemoryDB {
    fn clone(&self) -> Box<Database> {
        Box::new(Clone::clone(self))
    }

    fn snapshot(&self) -> Box<Snapshot> {
        Box::new(Clone::clone(self))
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

impl Snapshot for MemoryDB {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.map.get(key).map(Clone::clone)
    }

    fn contains(&self, key: &[u8]) -> bool {
        self.map.contains_key(key)
    }

    fn iter<'a>(&'a self, from: &[u8]) -> Iter<'a> {
        use std::collections::Bound::*;
        let range = (Included(from), Unbounded);
        Box::new(self.map.range::<[u8], _>(range).map(|(k, v)| (k.as_slice(), v.as_slice())))
    }
}
