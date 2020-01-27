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

use rocksdb::{WriteBatch, WriteOptions};
use tempfile::TempDir;

use std::sync::Arc;

use crate::backends::rocksdb::{RocksDB, RocksDBSnapshot};
use crate::{db::DB_METADATA, Database, DbOptions, Iter, Patch, ResolvedAddress, Result, Snapshot};

/// A wrapper over the `RocksDB` backend which stores data in the temporary directory
/// using the `tempfile` crate.
///
/// This database is only used for testing and experimenting; is not designed to
/// operate under load in production.
#[derive(Debug)]
pub struct TemporaryDB {
    inner: RocksDB,
    dir: Arc<TempDir>,
}

/// A wrapper over the `RocksDB` snapshot with the `TempDir` handle to prevent
/// it from destroying until all the snapshots and database itself are dropped.
struct TemporarySnapshot {
    snapshot: RocksDBSnapshot,
    _dir: Arc<TempDir>,
}

impl TemporaryDB {
    /// Creates a new, empty database.
    pub fn new() -> Self {
        let dir = Arc::new(TempDir::new().unwrap());
        let options = DbOptions::default();
        let inner = RocksDB::open(dir.path(), &options).unwrap();
        Self { dir, inner }
    }

    /// Clears the contents of the database.
    pub fn clear(&self) -> crate::Result<()> {
        /// Name of the default column family.
        const DEFAULT_CF: &str = "default";
        /// Some lexicographically large key.
        const LARGER_KEY: &[u8] = &[u8::max_value(); 1_024];

        let opts = rocksdb::Options::default();
        let names = rocksdb::DB::list_cf(&opts, self.dir.path())?;

        // For some reason, using a `WriteBatch` is significantly faster than using `DB::drop_cf`,
        // both in debug and release modes.
        let mut batch = WriteBatch::default();
        let db_reader = self.inner.get_lock_guard();
        for name in &names {
            if name != DEFAULT_CF && name != DB_METADATA {
                let cf_handle = db_reader.cf_handle(name).ok_or_else(|| {
                    let message = format!("Cannot access column family {}", name);
                    crate::Error::new(message)
                })?;
                let mut iter = db_reader.raw_iterator_cf(cf_handle)?;
                iter.seek_to_last();
                if iter.valid() {
                    if let Some(key) = iter.key() {
                        // For some reason, removing a range to a very large key is
                        // significantly faster than removing the exact range.
                        // This is specific to the debug mode, but since `TemporaryDB`
                        // is mostly used for testing, this optimization leads to practical
                        // performance improvement.
                        if key.len() < LARGER_KEY.len() {
                            batch.delete_range_cf::<&[u8]>(cf_handle, &[], LARGER_KEY)?;
                        } else {
                            batch.delete_range_cf::<&[u8]>(cf_handle, &[], &key)?;
                            batch.delete_cf(cf_handle, &key)?;
                        }
                    }
                }
            }
        }

        let write_options = WriteOptions::default();
        db_reader
            .write_opt(batch, &write_options)
            .map_err(Into::into)
    }

    fn temporary_snapshot(&self) -> TemporarySnapshot {
        TemporarySnapshot {
            snapshot: self.inner.rocksdb_snapshot(),
            _dir: Arc::clone(&self.dir),
        }
    }
}

impl Database for TemporaryDB {
    fn snapshot(&self) -> Box<dyn Snapshot> {
        Box::new(self.temporary_snapshot())
    }

    fn merge(&self, patch: Patch) -> Result<()> {
        self.inner.merge(patch)
    }

    fn merge_sync(&self, patch: Patch) -> Result<()> {
        self.inner.merge_sync(patch)
    }
}

impl Snapshot for TemporarySnapshot {
    fn get(&self, name: &ResolvedAddress, key: &[u8]) -> Option<Vec<u8>> {
        self.snapshot.get(name, key)
    }

    fn iter(&self, name: &ResolvedAddress, from: &[u8]) -> Iter<'_> {
        self.snapshot.iter(name, from)
    }
}

impl Default for TemporaryDB {
    fn default() -> Self {
        Self::new()
    }
}

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

#[test]
fn check_if_snapshot_is_still_valid() {
    use crate::access::CopyAccessExt;

    let (snapshot, db_path) = {
        let db = TemporaryDB::new();
        let db_path = db.dir.path().to_path_buf();
        let fork = db.fork();
        {
            let mut index = fork.get_list("index");
            index.push(1);
        }
        db.merge_sync(fork.into_patch()).unwrap();
        (db.snapshot(), db_path)
    };

    assert!(db_path.exists()); // A directory with db files still exists.
                               // So we can safety work with the snapshot.

    let index = snapshot.get_list("index");
    assert_eq!(index.get(0), Some(1));
}
