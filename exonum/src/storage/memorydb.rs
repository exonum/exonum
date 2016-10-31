use std::clone::Clone;
use std::sync::RwLock;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::btree_map;
use std::collections::Bound::{Included, Unbounded};
// use std::iter::Iterator;

use super::{Map, Database, Error, Patch, Change, Fork};
// use super::{Iterable, Seekable}

#[derive(Default)]
pub struct MemoryDB {
    map: RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
}
pub type MemoryDBIterator<'a> = btree_map::Iter<'a, Vec<u8>, Vec<u8>>;

pub struct MemoryDBView {
    map: MemoryDB,
    changes: RefCell<Patch>,
}

impl Clone for MemoryDB {
    fn clone(&self) -> MemoryDB {
        MemoryDB { map: RwLock::new(self.map.read().unwrap().clone()) }
    }
}

impl MemoryDB {
    pub fn new() -> MemoryDB {
        MemoryDB { map: RwLock::new(BTreeMap::new()) }
    }
}

impl Map<[u8], Vec<u8>> for MemoryDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.map.read().unwrap().get(key).cloned())
    }

    fn put(&self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
        self.map.write().unwrap().insert(key.to_vec(), value);
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), Error> {
        self.map.write().unwrap().remove(key);
        Ok(())
    }
    // TODO optimize me
    fn find_key(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let map = self.map.read().unwrap();
        let mut it = map.range::<[u8], [u8]>(Included(key), Unbounded);
        Ok(it.next().map(|x| x.0.to_vec()))
    }
}

impl MemoryDBView {
    fn new(from: &MemoryDB) -> MemoryDBView {
        MemoryDBView {
            map: from.clone(),
            changes: RefCell::default(),
        }
    }
}

impl Map<[u8], Vec<u8>> for MemoryDBView {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        match self.changes.borrow().get(key) {
            Some(change) => {
                let v = match *change {
                    Change::Put(ref v) => Some(v.clone()),
                    Change::Delete => None,
                };
                Ok(v)
            }
            None => self.map.get(key),
        }
    }

    fn put(&self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
        self.changes.borrow_mut().insert(key.to_vec(), Change::Put(value));
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), Error> {
        self.changes.borrow_mut().insert(key.to_vec(), Change::Delete);
        Ok(())
    }

    fn find_key(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        // TODO merge with the same function in memorydb
        let out = {
            let map = self.changes.borrow();
            let mut it = map.range::<[u8], [u8]>(Included(key), Unbounded);
            it.next().map(|x| x.0.to_vec())
        };
        if out.is_none() {
            self.map.find_key(key)
        } else {
            Ok(out)
        }
    }
}

impl Fork for MemoryDBView {
    fn changes(&self) -> Patch {
        self.changes.borrow().clone()
    }
    fn merge(&self, patch: &Patch) {
        let iter = patch.into_iter().map(|(k, v)| (k.clone(), v.clone()));
        self.changes.borrow_mut().extend(iter);
    }
}

impl Database for MemoryDB {
    type Fork = MemoryDBView;

    fn fork(&self) -> Self::Fork {
        MemoryDBView::new(self)
    }

    fn merge(&self, patch: &Patch) -> Result<(), Error> {
        let mut map = self.map.write().unwrap();
        for (key, change) in patch {
            match *change {
                Change::Put(ref v) => {
                    map.insert(key.clone(), v.clone());
                }
                Change::Delete => {
                    map.remove(key);
                }
            }
        }
        Ok(())
    }
}

// pub struct DatabaseIterator<'a> {
//     iter: MemoryDBIterator<'a>
// }

// impl<'a> Iterator for DatabaseIterator<'a> {
//     type Item = (Vec<u8>, Vec<u8>);

//     fn next(&mut self) -> Option<Self::Item> {
//         let item = self.iter.next();
//         item.map(|x| ((x.0.to_vec(), x.1.to_vec())))
//     }
// }

// impl<'a> Iterable for &'a MemoryDB {
//     type Iter = DatabaseIterator<'a>;

//     fn iter(self) -> Self::Iter {
//         DatabaseIterator {
//             iter: self.map.iter()
//         }
//     }
// }

// impl<'a> Seekable<'a> for DatabaseIterator<'a> {
//     type Key = Vec<u8>;
//     type Item = (Vec<u8>, Vec<u8>);

//     fn seek(&mut self, key: &Self::Key) -> Option<Self::Item> {
//         let opt = self.iter.find(|item| item.0 == key);
//         opt.map(|x| (x.0.to_vec(), x.1.to_vec()))
//     }
// }
