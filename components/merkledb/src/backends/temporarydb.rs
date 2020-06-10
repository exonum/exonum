// Copyright 2020 The Exonum Team
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

//! An implementation of `TemporaryDB` database.

use crossbeam::sync::ShardedLock;
use std::{
    collections::{btree_map::Range, BTreeMap, HashMap},
    iter::{Iterator, Peekable},
    ptr,
    sync::Arc,
};

use crate::{
    backends::rocksdb::{next_id_bytes, ID_SIZE},
    db::{check_database, Change, Iterator as DbIterator},
    Database, Iter, Patch, ResolvedAddress, Result, Snapshot,
};

type MemoryDB = HashMap<String, BTreeMap<Vec<u8>, Vec<u8>>>;

const DEFAULT_COLLECTION: &str = "default";

/// This in-memory database is only used for testing and experimenting; is not designed to
/// operate under load in production.
#[derive(Debug)]
pub struct TemporaryDB {
    inner: Arc<ShardedLock<MemoryDB>>,
}

struct TemporarySnapshot {
    snapshot: MemoryDB,
}

struct TemporaryDBIterator<'a> {
    iter: Peekable<Range<'a, Vec<u8>, Vec<u8>>>,
    prefix: Option<[u8; ID_SIZE]>,
    ended: bool,
}

impl TemporaryDB {
    /// Creates a new, empty database.
    pub fn new() -> Self {
        let mut db = HashMap::new();

        db.insert(DEFAULT_COLLECTION.to_owned(), BTreeMap::new());
        let inner = Arc::new(ShardedLock::new(db));
        let mut db = Self { inner };
        check_database(&mut db).unwrap();
        db
    }

    /// Clears the contents of the database.
    pub fn clear(&self) -> crate::Result<()> {
        let mut rw_lock = self.inner.write().expect("Couldn't get read-write lock");

        for collection in rw_lock.values_mut() {
            collection.clear();
        }

        Ok(())
    }

    fn temporary_snapshot(&self) -> TemporarySnapshot {
        TemporarySnapshot {
            snapshot: self.inner.read().expect("Couldn't get read lock").clone(),
        }
    }
}

impl Database for TemporaryDB {
    fn snapshot(&self) -> Box<dyn Snapshot> {
        Box::new(self.temporary_snapshot())
    }

    #[allow(unsafe_code)]
    fn merge(&self, patch: Patch) -> Result<()> {
        let mut inner = self.inner.write().expect("Couldn't get write lock");
        for (resolved, changes) in patch.into_changes() {
            if !inner.contains_key(&resolved.name) {
                inner.insert(resolved.name.clone(), BTreeMap::new());
            }

            let collection: &mut BTreeMap<Vec<u8>, Vec<u8>> =
                inner.get_mut(&resolved.name).unwrap();

            if changes.is_cleared() {
                if let Some(id_bytes) = resolved.id_to_bytes() {
                    let next_bytes = next_id_bytes(id_bytes).to_vec();
                    let keys_to_remove = collection
                        .range(id_bytes.to_vec()..next_bytes)
                        .map(|(key, _)| key.clone())
                        .collect::<Vec<_>>();
                    for key in keys_to_remove {
                        collection.remove(&key);
                    }
                } else {
                    collection.clear();
                }
            }

            if let Some(id_bytes) = resolved.id_to_bytes() {
                // Write changes to the column family with each key prefixed by the ID of the
                // resolved address.

                // We assume that typical key sizes are less than `1_024 - ID_SIZE = 1_016` bytes,
                // so that they fit into stack.
                let mut buffer: [u8; 1_024] = [0; 1024];
                unsafe {
                    ptr::copy(id_bytes.as_ptr(), buffer.as_mut_ptr(), ID_SIZE);
                }

                for (key, change) in changes.into_data() {
                    let buffer_size = key.len() + ID_SIZE;
                    unsafe {
                        ptr::copy(key.as_ptr(), buffer.as_mut_ptr().add(ID_SIZE), key.len());
                    }
                    match change {
                        Change::Put(value) => {
                            collection.insert(buffer[..buffer_size].into(), value)
                        }
                        Change::Delete => collection.remove(&buffer[..buffer_size]),
                    };
                }
            } else {
                // Write changes to the column family as-is.
                for (key, change) in changes.into_data() {
                    match change {
                        Change::Put(value) => collection.insert(key, value),
                        Change::Delete => collection.remove(&key),
                    };
                }
            }
        }
        Ok(())
    }

    fn merge_sync(&self, patch: Patch) -> Result<()> {
        self.merge(patch)
    }
}

impl<'a> DbIterator for TemporaryDBIterator<'a> {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        if self.ended {
            return None;
        }

        let (key, value) = self.iter.next()?;

        if let Some(ref prefix) = self.prefix {
            if &key[..ID_SIZE] != prefix {
                self.ended = true;
                return None;
            }
        }

        let key = if self.prefix.is_some() {
            &key[ID_SIZE..]
        } else {
            &key[..]
        };

        Some((key, value))
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        if self.ended {
            return None;
        }

        let (key, value) = self.iter.peek()?;
        let key = if let Some(prefix) = self.prefix {
            if key[..ID_SIZE] != prefix {
                self.ended = true;
                return None;
            }
            &key[ID_SIZE..]
        } else {
            &key[..]
        };

        Some((key, value))
    }
}

impl Snapshot for TemporarySnapshot {
    fn get(&self, name: &ResolvedAddress, key: &[u8]) -> Option<Vec<u8>> {
        let collection = self.snapshot.get(&name.name)?;
        collection.get(name.keyed(key).as_ref()).cloned()
    }

    fn iter(&self, name: &ResolvedAddress, from: &[u8]) -> Iter<'_> {
        let collection = self
            .snapshot
            .get(&name.name)
            .or_else(|| self.snapshot.get(DEFAULT_COLLECTION))
            .unwrap();
        let from = name.keyed(from).to_vec();
        let iter = collection.range(from..);

        Box::new(TemporaryDBIterator {
            iter: iter.peekable(),
            prefix: name.id_to_bytes(),
            ended: false,
        })
    }
}

impl Default for TemporaryDB {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::use_self)] // false positive
impl From<TemporaryDB> for Arc<dyn Database> {
    fn from(db: TemporaryDB) -> Self {
        Arc::new(db)
    }
}

#[test]
fn clearing_database() {
    use crate::access::CopyAccessExt;

    let db = TemporaryDB::new();
    let fork = db.fork();

    fork.get_list("foo").extend(vec![1_u32, 2, 3]);
    fork.get_proof_entry(("bar", &0_u8)).set("!".to_owned());
    fork.get_proof_entry(("bar", &1_u8)).set("?".to_owned());
    db.merge(fork.into_patch()).unwrap();
    db.clear().unwrap();

    let fork = db.fork();

    assert!(fork.index_type("foo").is_none());
    assert!(fork.index_type(("bar", &0_u8)).is_none());
    assert!(fork.index_type(("bar", &1_u8)).is_none());
    fork.get_proof_list("foo").extend(vec![4_u32, 5, 6]);
    db.merge(fork.into_patch()).unwrap();

    let snapshot = db.snapshot();
    let list = snapshot.get_proof_list::<_, u32>("foo");

    assert_eq!(list.len(), 3);
    assert_eq!(list.iter().collect::<Vec<_>>(), vec![4, 5, 6]);
}
