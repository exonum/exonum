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
use std::sync::Arc;
use std::clone::Clone;

use tempdir::TempDir;

use super::{Database, View, RocksDB, RocksDBOptions};

/// Database implementation that stores all the data in memory.
///
/// It's mainly used for testing and not designed to be efficient.
#[derive(Debug, Clone)]
pub struct MemoryDB {
    db: RocksDB,
    tmp_dir: Arc<TempDir>,
}

impl MemoryDB {
    /// Creates a new, empty database.
    pub fn new() -> Self {
        let tmp_dir = TempDir::new("tmpdir").unwrap();
        let mut opts = RocksDBOptions::default();
        opts.create_if_missing(true);
        opts.set_use_fsync(false);
        MemoryDB {
            db: RocksDB::open(tmp_dir.path(), opts).unwrap(),
            tmp_dir: Arc::new(tmp_dir),
        }
    }
}

impl Default for MemoryDB {
    fn default() -> Self {
        Self::new()
    }
}

impl Database for MemoryDB {
    fn clone(&self) -> Box<Database> {
        Box::new(Clone::clone(self))
    }

    fn snapshot(&self) -> Arc<View> {
        self.db.snapshot()
    }

    fn fork(&self) -> Arc<View> {
        self.db.fork()
    }
}

#[test]
fn test_memorydb_snapshot() {
    let db = MemoryDB::new();

    {
        let fork = db.fork();
        fork.put("a", &[1, 2, 3], &[123]);
        fork.commit();
    }

    let snapshot = db.snapshot();
    assert!(snapshot.contains("a", &[1, 2, 3]));

    {
        let fork = db.fork();
        fork.put("a", &[2, 3, 4], &[234]);
        fork.commit();
    }

    assert!(!snapshot.contains("a", &[2, 3, 4]));

    let snapshot = db.snapshot();
    assert!(snapshot.contains("a", &[2, 3, 4]));
}
