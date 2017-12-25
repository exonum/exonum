// Copyright 2017 The Exonum Team
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
use std::sync::{Arc, RwLock};
use std::clone::Clone;
use std::collections::btree_map::BTreeMap;

use super::{Database, Snapshot, Patch, Iterator, Iter, Result};
use super::db::Change;

type DB = BTreeMap<Vec<u8>, Vec<u8>>;

/// Database implementation that stores all the data in memory.
///
/// It's mainly used for testing and not designed to be efficient.
#[derive(Default, Clone, Debug)]
pub struct MemoryDB {
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
        MemoryDB { map: Arc::new(RwLock::new(BTreeMap::new())) }
    }
}

impl Database for MemoryDB {
    fn clone(&self) -> Box<Database> {
        Box::new(Clone::clone(self))
    }

    fn snapshot(&self) -> Box<Snapshot> {
        Box::new(MemoryDB {
            map: Arc::new(RwLock::new(self.map.read().unwrap().clone())),
        })
    }

    fn merge(&mut self, patch: Patch) -> Result<()> {
        let mut guarded_db = self.map.write().unwrap();
        for (cf_name, changes) in patch {
            for (key, change) in changes {
                let prefixed_key = generate_key(&cf_name, &key);
                match change {
                    Change::Put(ref value) => {
                        guarded_db.insert(prefixed_key, value.to_vec());
                    }
                    Change::Delete => {
                        guarded_db.remove(&prefixed_key);
                    }
                }
            }
        }
        Ok(())
    }

    fn merge_sync(&mut self, patch: Patch) -> Result<()> {
        self.merge(patch)
    }
}

impl Snapshot for MemoryDB {
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>> {
        let prefixed_key = generate_key(name, key);
        self.map.read().unwrap().get(&prefixed_key).cloned()
    }

    fn contains(&self, name: &str, key: &[u8]) -> bool {
        let prefixed_key = generate_key(name, key);
        self.map.read().unwrap().contains_key(&prefixed_key)
    }

    fn iter(&self, name: &str, from: &[u8]) -> Iter {
        let prefixed_from = generate_key(name, from);
        let data = self.map
            .read()
            .unwrap()
            .iter()
            .skip_while(|&(k, _)| k < &prefixed_from)
            .filter(|&(k, _)| k.len() >= prefixed_from.len())
            .filter(|&(k, _)| &k[0..name.len()] == name.as_bytes())
            .map(|(k, v)| (k[name.len()..].to_vec(), v.to_vec()))
            .collect::<Vec<_>>();

        Box::new(MemoryDBIter { data, index: 0 })
    }
}

impl Iterator for MemoryDBIter {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        if self.index < self.data.len() {
            self.index += 1;
            self.data.get(self.index - 1).map(|&(ref k, ref v)| {
                (k.as_slice(), v.as_slice())
            })
        } else {
            None
        }
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        if self.index < self.data.len() {
            self.data.get(self.index).map(|&(ref k, ref v)| {
                (k.as_slice(), v.as_slice())
            })
        } else {
            None
        }
    }
}

fn generate_key(name: &str, key: &[u8]) -> Vec<u8> {
    let mut prefixed_key = Vec::from(name.as_bytes());
    prefixed_key.extend(key);
    prefixed_key
}

#[test]
fn test_memorydb_snapshot() {
    let mut db = MemoryDB::new();
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

    {
        let db_clone = Clone::clone(&db);
        let snap_clone = db_clone.snapshot();
        assert!(snap_clone.contains(idx_name, vec![2, 3, 4].as_slice()));
    }

    let snapshot = db.snapshot();
    assert!(snapshot.contains(idx_name, vec![2, 3, 4].as_slice()));
}

#[test]
fn test_generate_prefixed_key() {
    let prefixed_key = generate_key("abc", &[1, 2]);
    assert_eq!(&prefixed_key, &[97, 98, 99, 1, 2]);
}
