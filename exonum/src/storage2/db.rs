use std::collections::BTreeMap;

use super::Result;

pub type Patch = BTreeMap<Vec<u8>, Change>;

#[derive(Clone)]
pub enum Change {
    Put(Vec<u8>),
    Delete,
}

pub struct Fork {
    snapshot: Box<Snapshot>,
    changes: Patch
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
}

impl Fork {
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.changes.insert(key, Change::Put(value));
    }

    pub fn delete(&mut self, key: Vec<u8>) {
        self.changes.insert(key, Change::Delete);
    }

    pub fn into_patch(self) -> Patch {
        self.changes
    }
}
