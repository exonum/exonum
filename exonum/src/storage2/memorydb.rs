use std::clone::Clone;
use std::collections::BTreeMap;

use super::{Database, Snapshot, Result, Patch, Change, Fork, Iter};

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

impl Snapshot for MemoryDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.map.get(key).map(Clone::clone))
    }

    fn contains(&self, key: &[u8]) -> Result<bool> {
        Ok(self.map.contains_key(key))
    }

    fn iter<'a>(&'a self, from: Option<&[u8]>) -> Iter<'a> {
        use std::collections::Bound::*;
        let range = if let Some(seek) = from {
            (Included(seek), Unbounded)
        } else {
            (Unbounded, Unbounded)
        };
        Box::new(self.map.range::<[u8], _>(range).map(|(k, v)| (k.as_slice(), v.as_slice())))
    }
}
