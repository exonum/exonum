// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! An implementation of `MemoryDB` database.

use std::{clone::Clone,
          collections::{BTreeMap, HashMap},
          sync::{Arc, RwLock}};

use storage;
use super::{db::Change, Database, Error, DbView, DbViewMut, Iter, Iterator, Patch, Result};

type DB = HashMap<String, BTreeMap<Vec<u8>, Vec<u8>>>;

/// Database implementation that stores all the data in memory.
///
/// It's mainly used for testing and not designed to be efficient.
#[derive(Default, Debug)]
pub struct MemoryDB {
    map: Arc<RwLock<DB>>,
}

pub struct MemoryDBSnapshot {
    map: Arc<RwLock<DB>>,
}

/// An iterator over the entries of a `MemoryDB`.
struct MemoryDBIter {
    data: Vec<(Vec<u8>, Vec<u8>)>,
    index: usize,
}

impl MemoryDB {
    /// Creates a new, empty database.
    pub fn new() -> MemoryDB {
        MemoryDB {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Database for MemoryDB {
    fn snapshot(&self) -> Box<DbView> {
        Box::new(MemoryDB {
            map: Arc::new(RwLock::new(self.map.read().unwrap().clone())),
        })
    }

    fn view(&self) -> Box<DbView> {
        Box::new(MemoryDB {
            map: self.map.clone(),
        })
    }

    fn merge(&self, patch: Patch) -> Result<()> {
        let mut guard = self.map.write().unwrap();
        for (cf_name, changes) in patch {
            if !guard.contains_key(&cf_name) {
                guard.insert(cf_name.clone(), BTreeMap::new());
            }
            let table = guard.get_mut(&cf_name).unwrap();
            for (key, change) in changes {
                match change {
                    Change::Put(ref value) => {
                        table.insert(key, value.to_vec());
                    }
                    Change::Delete => {
                        table.remove(&key);
                    }
                }
            }
        }
        Ok(())
    }

    fn merge_sync(&self, patch: Patch) -> Result<()> {
        self.merge(patch)
    }
}

impl DbView for MemoryDB {
    fn get(&self, cf_name: &str, key: &[u8]) -> Option<Vec<u8>> {
        self.map
            .read()
            .unwrap()
            .get(cf_name)
            .and_then(|table| table.get(&key.to_vec()).cloned())
    }

    fn contains(&self, cf_name: &str, key: &[u8]) -> bool {
        self.map
            .read()
            .unwrap()
            .get(cf_name)
            .map_or(false, |table| table.contains_key(&key.to_vec()))
    }

    fn iter<'a>(&'a self, cf_name: &str, from: &[u8]) -> Iter<'a> {
        let map_guard = self.map.read().unwrap();
        let data = match map_guard.get(cf_name) {
            Some(table) => table
                .iter()
                .skip_while(|&(k, _)| k.as_slice() < from)
                .map(|(k, v)| (k.to_vec(), v.to_vec()))
                .collect(),
            None => Vec::new(),
        };

        Box::new(MemoryDBIter { data, index: 0 })
    }
}

impl DbViewMut for MemoryDB {
    fn put(&mut self, cf_name: &str, key: Vec<u8>, value: Vec<u8>) -> storage::Result<()> {
        self.map
            .write()
            .unwrap()
            .entry(cf_name.to_string())
            .or_insert(BTreeMap::new())
            .insert(key, value);
        Ok(())
    }

    fn remove(&mut self, cf_name: &str, key: Vec<u8>) -> storage::Result<()> {
        let mut map = self.map
            .write()
            .unwrap();
        let mut column_family = map.get_mut(cf_name);
        match column_family {
            Some(ref mut cf) => {
                cf.remove(&key);
                Ok(())
            },
            None => Err(Error::new(format!(
                "There is no column family with such name: {}",
                cf_name,
            )))
        }
    }

    fn remove_by_prefix(&mut self, cf_name: &str, prefix: Option<&Vec<u8>>) -> storage::Result<()> {
        let mut map = self.map
            .write()
            .unwrap();
        let column_family = map.remove(cf_name);
        match column_family {
            Some(cf) => {
                let cf = match prefix {
                    Some(prefix) => {
                        cf.into_iter()
                            .filter(|(k, _)| !k.starts_with(prefix))
                            .collect::<BTreeMap<_, _>>()
                    },
                    None => BTreeMap::new(),
                };
                map.insert(cf_name.to_string(), cf);
                Ok(())
            },
            None => Err(Error::new(format!(
                "There is no column family with such name: {}",
                cf_name,
            )))
        }
    }
}

impl DbView for MemoryDBSnapshot {
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>> {
        self.map
            .read()
            .unwrap()
            .get(name)
            .and_then(|table| table.get(key).cloned())
    }

    fn contains(&self, name: &str, key: &[u8]) -> bool {
        self.map
            .read()
            .unwrap()
            .get(name)
            .map_or(false, |table| table.contains_key(key))
    }

    fn iter(&self, name: &str, from: &[u8]) -> Iter {
        let map_guard = self.map.read().unwrap();
        let data = match map_guard.get(name) {
            Some(table) => table
                .iter()
                .skip_while(|&(k, _)| k.as_slice() < from)
                .map(|(k, v)| (k.to_vec(), v.to_vec()))
                .collect(),
            None => Vec::new(),
        };

        Box::new(MemoryDBIter { data, index: 0 })
    }
}

impl Iterator for MemoryDBIter {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        if self.index < self.data.len() {
            self.index += 1;
            self.data
                .get(self.index - 1)
                .map(|&(ref k, ref v)| (k.as_slice(), v.as_slice()))
        } else {
            None
        }
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        if self.index < self.data.len() {
            self.data
                .get(self.index)
                .map(|&(ref k, ref v)| (k.as_slice(), v.as_slice()))
        } else {
            None
        }
    }
}

impl From<MemoryDB> for Arc<Database> {
    fn from(db: MemoryDB) -> Arc<Database> {
        Arc::from(Box::new(db) as Box<Database>)
    }
}

#[test]
fn test_memorydb_snapshot() {
    let db = MemoryDB::new();
    let idx_name = "idx_name";
    {
        let mut fork = db.fork();
        fork.put(idx_name, vec![1, 2, 3], vec![123]);
        let _ = db.merge(fork.into_patch());
    }

    let snapshot = db.snapshot();
    assert!(snapshot.contains(idx_name, vec![1, 2, 3].as_slice()));

    {
        let mut fork = db.fork();
        fork.put(idx_name, vec![2, 3, 4], vec![234]);
        let _ = db.merge(fork.into_patch());
    }

    assert!(!snapshot.contains(idx_name, vec![2, 3, 4].as_slice()));

    let snapshot = db.snapshot();
    assert!(snapshot.contains(idx_name, vec![2, 3, 4].as_slice()));
}
