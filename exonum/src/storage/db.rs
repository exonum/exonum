use std::collections::BTreeMap;
use std::collections::Bound::{Included, Unbounded};

use super::{Map, Error};

// TODO In this implementation there are extra memory allocations when key is passed into specific database.
// Think about key type. Maybe we can use keys with fixed length?
pub trait Database: Map<[u8], Vec<u8>> + Sized {
    fn fork<'a>(&'a self) -> Fork<'a, Self> {
        Fork {
            database: self,
            changes: BTreeMap::new(),
        }
    }

    fn merge(&mut self, patch: Patch) -> Result<(), Error>;
}

pub enum Change {
    Put(Vec<u8>),
    Delete,
}

pub type Patch = BTreeMap<Vec<u8>, Change>;

pub struct Fork<'a, T: Database + 'a> {
    database: &'a T,
    changes: Patch,
}

impl<'a, T: Database + 'a> From<Fork<'a, T>> for Patch {
    fn from(fork: Fork<'a, T>) -> Patch {
        fork.changes
    }
}

impl<'a, T> Map<[u8], Vec<u8>> for Fork<'a, T>
    where T: Database + 'a
{
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        match self.changes.get(key) {
            Some(change) => {
                let v = match *change {
                    Change::Put(ref v) => Some(v.clone()),
                    Change::Delete => None,
                };
                Ok(v)
            }
            None => self.database.get(key),
        }
    }

    fn put(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
        self.changes.insert(key.to_vec(), Change::Put(value));
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), Error> {
        self.changes.insert(key.to_vec(), Change::Delete);
        Ok(())
    }

    fn find_key(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        //TODO merge with the same function in memorydb
        let out = {
            let mut it = self.changes.range::<[u8], [u8]>(Included(key), Unbounded);
            it.next().map(|x| x.0.to_vec())
        };
        if out.is_none() {
            return self.database.find_key(key);
        } else {
            return Ok(out);
        }
    }
}

impl<'a, T: Database + 'a + ?Sized> Database for Fork<'a, T> {
    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        self.changes.extend(patch.into_iter());
        Ok(())
    }
}
