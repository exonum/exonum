use std::iter::Iterator;
use std::collections::btree_map::{BTreeMap, Range};

use super::Result;

pub type Patch = BTreeMap<Vec<u8>, Change>;
pub type Iter<'a> = Box<Iterator<Item=(&'a [u8], &'a [u8])> + 'a>;

#[derive(Clone)]
pub enum Change {
    Put(Vec<u8>),
    Delete,
}

pub struct Fork {
    snapshot: Box<Snapshot>,
    changes: Patch
}

pub struct ForkIter<'a> {
    snapshot: Iter<'a>,
    changes: Range<'a, Vec<u8>, Change>
}

pub trait Database: Sized + Clone + Send + Sync + 'static {
    fn snapshot(&self) -> Box<Snapshot>;
    fn fork(&self) -> Fork {
        Fork {
            snapshot: self.snapshot(),
            changes: Patch::new(),
        }
    }
    fn merge(&mut self, patch: Patch) -> Result<()>;
}

pub trait Snapshot {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    fn contains(&self, key: &[u8]) -> Result<bool> {
        Ok(self.get(key)?.is_some())
    }
    fn iter<'a>(&'a self, from: Option<&[u8]>) -> Iter<'a>;
}

impl Snapshot for Fork {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self.changes.get(key) {
            Some(change) => Ok(match *change {
                Change::Put(ref v) => Some(v.clone()),
                Change::Delete => None,
            }),
            None => self.snapshot.get(key)
        }
    }

    fn contains(&self, key: &[u8]) -> Result<bool> {
        Ok(match self.changes.get(key) {
            Some(change) => match *change {
                Change::Put(..) => true,
                Change::Delete => false,
            },
            None => self.snapshot.get(key)?.is_some()
        })
    }

    fn iter<'a>(&'a self, from: Option<&[u8]>) -> Iter<'a> {
        use std::collections::Bound::*;
        Box::new(ForkIter {
            snapshot: self.snapshot.iter(from),
            changes: if let Some(seek) = from {
                self.changes.range::<[u8], _>((Included(seek), Unbounded))
            } else {
                self.changes.range::<[u8], _>(..)
            }
        })
    }
}

impl Fork {
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.changes.insert(key, Change::Put(value));
    }

    pub fn delete(&mut self, key: Vec<u8>) {
        self.changes.insert(key, Change::Delete);
    }

    pub fn as_snapshot(&self) -> &Snapshot {
        &*self.snapshot
    }

    pub fn into_patch(self) -> Patch {
        self.changes
    }
}

impl<'a> Iterator for ForkIter<'a> {
    type Item = (&'a [u8], &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        self.snapshot.next()
    }
}
