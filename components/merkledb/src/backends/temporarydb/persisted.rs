// Copyright 2022 The Exonum Team
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

//! An implementation of `TemporaryDB` database over `RockDB`.

use tempfile::TempDir;

use crate::{Database, DbOptions, Patch, Result, RocksDB, Snapshot};

/// This database is only used for testing and experimenting; is not designed to
/// operate under load in production.
#[derive(Debug)]
pub struct TemporaryDB {
    db: RocksDB,
    dir: TempDir,
}

impl TemporaryDB {
    /// Creates a new, temporary, empty database.
    pub fn new() -> Self {
        let dir = TempDir::new().expect("Couldn't create temp directory");
        let db = RocksDB::open(dir.path(), &DbOptions::default())
            .expect("Couldn't create temporary database");

        Self { db, dir }
    }

    /// Clears the contents of the database.
    pub fn clear(&self) -> Result<()> {
        let opts = rocksdb::Options::default();
        let cf_names = rocksdb::DB::list_cf(&opts, self.dir.path())?;
        let db = self.db.get_inner();

        for cf_name in cf_names {
            let mut batch = rocksdb::WriteBatch::default();

            if let Some(cf) = db.cf_handle(&cf_name) {
                self.db.clear_column_family(&mut batch, &cf);
                db.write(batch)?;
                db.flush_cf(&cf)?;
            }
        }

        Ok(())
    }
}

impl Database for TemporaryDB {
    fn snapshot(&self) -> Box<dyn Snapshot> {
        self.db.snapshot()
    }

    fn merge(&self, patch: Patch) -> Result<()> {
        self.db.merge(patch)
    }

    fn merge_sync(&self, patch: Patch) -> Result<()> {
        self.db.merge_sync(patch)
    }
}
