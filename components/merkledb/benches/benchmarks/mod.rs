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

use exonum_merkledb::{DbOptions, RocksDB};
use tempfile::{tempdir, TempDir};

pub mod encoding;
pub mod schema_patterns;
pub mod storage;
pub mod transactions;

pub(super) struct BenchDB {
    _dir: TempDir,
    db: RocksDB,
}

impl AsRef<RocksDB> for BenchDB {
    fn as_ref(&self) -> &RocksDB {
        &self.db
    }
}

pub(super) fn create_database() -> BenchDB {
    let dir = tempdir().expect("Couldn't create tempdir");
    let db = RocksDB::open(dir.path(), &DbOptions::default()).expect("Couldn't create database");
    BenchDB { _dir: dir, db }
}
