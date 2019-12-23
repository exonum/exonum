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

use super::rocksdb::RocksDB;
use crate::{db::DB_METADATA, Database, DbOptions, Patch, Result, Snapshot};

/// Wrapper over the `RocksDB` backend which stores data in the temporary directory
/// using the `tempfile` crate.
///
/// This database is only used for testing and experimenting; is not designed to
/// operate under load in production.
#[derive(Debug)]
pub struct TemporaryDB {
    inner: RocksDB,
    dir: TempDir,
}

impl TemporaryDB {
    /// Creates a new, empty database.
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let options = DbOptions::default();
        let inner = RocksDB::open(&dir, &options).unwrap();
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
        let db = self.inner.rocksdb();
        let db_reader = db.read().expect("Couldn't get read lock to DB");
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
}

impl Database for TemporaryDB {
    fn snapshot(&self) -> Box<dyn Snapshot> {
        self.inner.snapshot()
    }

    fn merge(&self, patch: Patch) -> Result<()> {
        self.inner.merge(patch)
    }

    fn merge_sync(&self, patch: Patch) -> Result<()> {
        self.inner.merge_sync(patch)
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
    use crate::access::AccessExt;

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
