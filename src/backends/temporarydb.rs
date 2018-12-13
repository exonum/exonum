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

//! An implementation of `TemporaryDB` database.

use tempfile::TempDir;

use super::rocksdb::RocksDB;
use crate::{Database, DbOptions, Patch, Result, Snapshot};

/// Wrapper over the `RocksDB` backend which stores data in the temporary directory
/// using the `tempfile` crate.
///
/// This database is only used for testing and experimenting; is not designed to
/// operate under load in production.
pub struct TemporaryDB {
    inner: RocksDB,
    _dir: TempDir,
}

impl TemporaryDB {
    /// Creates a new, empty database.
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let options = DbOptions::default();
        let inner = RocksDB::open(&dir, &options).unwrap();
        Self { _dir: dir, inner }
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
