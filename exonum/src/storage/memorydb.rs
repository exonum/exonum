use std::sync::RwLock;
use std::collections::BTreeMap;
use std::collections::btree_map;
use std::collections::Bound::{Included, Unbounded};
// use std::iter::Iterator;

use super::{Map, Database, Error, Patch, Change};
// use super::{Iterable, Seekable}

#[derive(Default)]
pub struct MemoryDB {
    map: RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
}
pub type MemoryDBIterator<'a> = btree_map::Iter<'a, Vec<u8>, Vec<u8>>;

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

impl Database for MemoryDB {
    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        let mut map = self.map.write().unwrap();
        for (key, change) in patch.into_iter() {
            match change {
                Change::Put(ref v) => {
                    map.insert(key.clone(), v.clone());
                }
                Change::Delete => {
                    map.remove(&key);
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
