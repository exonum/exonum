use std::collections::BTreeMap;

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

impl<'a, T: Database + 'a> Fork<'a, T> {
    pub fn patch(self) -> Patch {
        self.changes
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
}

impl<'a, T: Database + 'a + ?Sized> Database for Fork<'a, T> {
    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        self.changes.extend(patch.into_iter());
        Ok(())
    }
}
