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

use exonum_merkledb::{Database, DbOptions, Fork, Patch, Result, RocksDB, Snapshot};
use tempfile::{tempdir, TempDir};

pub mod encoding;
pub mod schema_patterns;
pub mod storage;
pub mod transactions;

pub(super) struct BenchDB {
    _dir: TempDir,
    db: RocksDB,
}

impl BenchDB {
    pub(crate) fn new() -> Self {
        let dir = tempdir().expect("Couldn't create tempdir");
        let db =
            RocksDB::open(dir.path(), &DbOptions::default()).expect("Couldn't create database");
        Self { _dir: dir, db }
    }

    pub(crate) fn fork(&self) -> Fork {
        self.db.fork()
    }

    pub(crate) fn snapshot(&self) -> Box<dyn Snapshot> {
        self.db.snapshot()
    }

    pub(crate) fn merge(&self, patch: Patch) -> Result<()> {
        self.db.merge(patch)
    }

    pub(crate) fn merge_sync(&self, patch: Patch) -> Result<()> {
        self.db.merge_sync(patch)
    }
}

impl Default for BenchDB {
    fn default() -> Self {
        Self::new()
    }
}
