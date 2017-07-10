//! An implementation of `MemoryDB` database.
use std::clone::Clone;
use std::collections::btree_map::{BTreeMap, Range};
use std::iter::Peekable;

use super::{Database, Snapshot, Patch, Change, Iterator, Iter, Result};

/// Database implementation that stores all the data in memory.
///
/// It's mainly used for testing and not designed to be efficient.
#[derive(Default, Clone, Debug)]
pub struct MemoryDB {
    map: BTreeMap<Vec<u8>, Vec<u8>>,
}

/// An iterator over an entries of a `MemoryDB`.
struct MemoryDBIter<'a> {
    iter: Peekable<Range<'a, Vec<u8>, Vec<u8>>>,
}

impl MemoryDB {
    /// Creates a new, empty database.
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
                Change::Put(value) => {
                    self.map.insert(key, value);
                }
                Change::Delete => {
                    self.map.remove(&key);
                }
            }
        }
        Ok(())
    }
}

impl Snapshot for MemoryDB {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.map.get(key).cloned()
    }

    fn contains(&self, key: &[u8]) -> bool {
        self.map.contains_key(key)
    }

    fn iter<'a>(&'a self, from: &[u8]) -> Iter<'a> {
        use std::collections::Bound::*;
        let range = (Included(from), Unbounded);
        Box::new(MemoryDBIter { iter: self.map.range::<[u8], _>(range).peekable() })
    }
}

impl<'a> Iterator for MemoryDBIter<'a> {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        self.iter
            .next()
            .map(|(k, v)| (k.as_slice(), v.as_slice()))
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        self.iter
            .peek()
            .map(|&(k, v)| (k.as_slice(), v.as_slice()))
    }
}
