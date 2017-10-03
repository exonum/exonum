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

//! An implementation of `RocksDB` database.
use exonum_profiler::ProfilerSpan;
use rocksdb::OptimisticTransactionDB as _RocksDB;
use rocksdb::DBRawIterator;
use rocksdb::optimistic_txn_db::Snapshot as _Snapshot;
use rocksdb::Transaction as _Transaction;
use rocksdb::Error as _Error;
use rocksdb::IteratorMode;
use rocksdb::utils::get_cf_names;

use std::sync::{Arc, RwLock};
use std::path::Path;
use std::fmt;
use std::error;

pub use rocksdb::WriteBatch as Patch;
pub use rocksdb::Options as RocksDBOptions;
pub use rocksdb::BlockBasedOptions as RocksBlockOptions;
pub use rocksdb::{OptimisticTransactionOptions, WriteOptions, ColumnFamily};

use super::{Database, Iterator, Iter, View, Error, Result};

impl From<_Error> for Error {
    fn from(err: _Error) -> Self {
        Error::new(error::Error::description(&err))
    }
}

/// Database implementation on the top of `RocksDB` backend.
#[derive(Clone)]
pub struct RocksDB {
    db: Arc<RwLock<_RocksDB>>,
}

/// A snapshot of a `RocksDB`.
pub struct RocksDBSnapshot {
    snapshot: _Snapshot,
    _db: Arc<RwLock<_RocksDB>>,
}

/// A transaction of a `RocksDB`
#[derive(Clone)]
pub struct RocksDBTransaction {
    transaction: _Transaction,
    _db: Arc<RwLock<_RocksDB>>,
}

/// An iterator over the entries of a `RocksDB`.
struct RocksDBIterator {
    iter: DBRawIterator,
    key: Option<Vec<u8>>,
    value: Option<Vec<u8>>,
}

impl RocksDB {
    /// Open a database stored in the specified path with the specified options.
    pub fn open(path: &Path, options: RocksDBOptions) -> Result<RocksDB> {
        let db = {
            if let Ok(names) = get_cf_names(path) {
                let cf_names = names.iter().map(|name| name.as_str()).collect::<Vec<_>>();
                _RocksDB::open_cf(&options, path, cf_names.as_ref())?
            } else {
                _RocksDB::open(&options, path)?
            }
        };
        Ok(RocksDB { db: Arc::new(RwLock::new(db)) })
    }

    /// Destroy database
    pub fn destroy<P: AsRef<Path>>(path: P) {
        let _ = _RocksDB::destroy(&RocksDBOptions::default(), path);
    }
}

impl Database for RocksDB {
    fn clone(&self) -> Box<Database> {
        Box::new(Clone::clone(self))
    }

    fn snapshot(&self) -> Arc<View> {
        let _p = ProfilerSpan::new("RocksDB::snapshot");
        Arc::new(RocksDBSnapshot {
            snapshot: self.db.read().unwrap().snapshot(),
            _db: self.db.clone(),
        })
    }

    fn fork(&self) -> Arc<View> {
        let _p = ProfilerSpan::new("RocksDB::transaction");
        let w_opts = WriteOptions::default();
        let txn_opts = OptimisticTransactionOptions::default();
        Arc::new(RocksDBTransaction {
            transaction: self.db.read().unwrap().transaction_begin(
                &w_opts,
                &txn_opts,
            ),
            _db: self.db.clone(),
        })
    }
}

impl View for RocksDBSnapshot {
    fn get(&self, cf_name: &str, key: &[u8]) -> Option<Vec<u8>> {
        let _p = ProfilerSpan::new("RocksDBSnapshot::get");
        if let Some(column_family) = self._db.read().unwrap().cf_handle(cf_name) {
            match self.snapshot.get_cf(column_family, key) {
                Ok(value) => value.map(|v| v.to_vec()),
                Err(e) => panic!(e),
            }
        } else {
            None
        }
    }

    fn iter(&self, cf_name: &str, from: Option<&[u8]>) -> Iter {
        let _p = ProfilerSpan::new("RocksDBSnapshot::iter");
        let iter = match self._db.read().unwrap().cf_handle(cf_name) {
            Some(column_family) => {
                let mut iter: DBRawIterator = self.snapshot
                    .iterator_cf(column_family, IteratorMode::Start)
                    .unwrap()
                    .into();
                if let Some(f) = from {
                    iter.seek(f);
                }
                iter
            }
            None => self.snapshot.raw_iterator(),
        };
        Box::new(RocksDBIterator {
            iter,
            key: None,
            value: None,
        })
    }

    fn put(&self, _: &str, _: &[u8], _: &[u8]) {
        panic!("PUT is unsupported. Snapshot is read only");
    }

    fn delete(&self, _: &str, _: &[u8]) {
        panic!("DELETE is unsupported. Snapshot is read only");
    }

    fn clear(&self, _: &str) {
        panic!("CLEAR is unsupported. Snapshot is read only");
    }

    fn commit(&self) {
        panic!("COMMIT is unsupported. Snapshot is read only");
    }

    fn rollback(&self) {
        panic!("ROLLBACK is unsupported. Snapshot is read only");
    }

    fn savepoint(&self) {
        panic!("SAVEPOINT is unsupported. Snapshot is read only");
    }

    fn rollback_to_savepoint(&self) {
        panic!("ROLLBACK_TO_SAVEPOINT is unsupported. Snapshot is read only");
    }
}

impl RocksDBTransaction {
    fn delete_cf(&self, cf: ColumnFamily, key: &[u8]) {
        if let Err(e) = self.transaction.delete_cf(cf, key) {
            error!("Error while deleting, {}", e);
        }
    }

    fn get_column_family(&self, cf_name: &str) -> Option<ColumnFamily> {
        self._db.read().unwrap().cf_handle(cf_name)
    }
}

impl View for RocksDBTransaction {
    fn get(&self, cf_name: &str, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(column_family) = self.get_column_family(cf_name) {
            match self.transaction.get_cf(column_family, key) {
                Ok(Some(value)) => Some(value.iter().cloned().collect::<Vec<_>>()),
                _ => None,
            }
        } else {
            None
        }
    }

    fn put(&self, cf_name: &str, key: &[u8], value: &[u8]) {
        let cf = self.get_column_family(cf_name);
        let column_family = match cf {
            Some(cf) => cf,
            None => {
                let opts = RocksDBOptions::default();
                match self._db.write().unwrap().create_cf(cf_name, &opts) {
                    Ok(cf) => cf,
                    Err(e) => {
                        panic!("Error while creating column family: {}", e);
                    }
                }
            }
        };
        if let Err(e) = self.transaction.put_cf(column_family, key, value) {
            error!("Error while putting, {}", e);
        }
    }

    fn delete(&self, cf_name: &str, key: &[u8]) {
        if let Some(column_family) = self.get_column_family(cf_name) {
            self.delete_cf(column_family, key);
        }
    }

    fn clear(&self, cf_name: &str) {
        if let Some(column_family) = self.get_column_family(cf_name) {
            let mut iter = self.iter(cf_name, None);
            while let Some((key, _)) = iter.next() {
                self.delete_cf(column_family, key);
            }
        }
    }

    fn iter(&self, cf_name: &str, from: Option<&[u8]>) -> Iter {
        let _p = ProfilerSpan::new("RocksDBTransaction::iter");
        let iter = match self.get_column_family(cf_name) {
            Some(column_family) => {
                let mut iter: DBRawIterator =
                    self.transaction.iterator_cf(column_family).unwrap().into();
                if let Some(f) = from {
                    iter.seek(f);
                }
                iter
            }
            None => self.transaction.iterator().into(),
        };
        Box::new(RocksDBIterator {
            iter,
            key: None,
            value: None,
        })
    }

    fn commit(&self) {
        if let Err(e) = self.transaction.commit() {
            error!("Commit error, {}", e);
        }
    }

    fn rollback(&self) {
        if let Err(e) = self.transaction.rollback() {
            error!("Rollback error: {}", e);
        }
    }

    fn savepoint(&self) {
        self.transaction.savepoint();
    }

    fn rollback_to_savepoint(&self) {
        if let Err(e) = self.transaction.rollback_to_savepoint() {
            error!("Rollback to save point error: {}", e);
        }
    }
}

impl<'a> Iterator for RocksDBIterator {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        let _p = ProfilerSpan::new("RocksDBIterator::next");
        let result = if self.iter.valid() {
            self.key = Some(unsafe { self.iter.key_inner().unwrap().to_vec() });
            self.value = Some(unsafe { self.iter.value_inner().unwrap().to_vec() });
            Some((
                self.key.as_ref().unwrap().as_ref(),
                self.value.as_ref().unwrap().as_ref(),
            ))
        } else {
            None
        };

        if result.is_some() {
            self.iter.next();
        }

        result
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        let _p = ProfilerSpan::new("RocksDBIterator::peek");
        if self.iter.valid() {
            self.key = Some(unsafe { self.iter.key_inner().unwrap().to_vec() });
            self.value = Some(unsafe { self.iter.value_inner().unwrap().to_vec() });
            Some((
                self.key.as_ref().unwrap().as_ref(),
                self.value.as_ref().unwrap().as_ref(),
            ))
        } else {
            None
        }
    }
}

impl fmt::Debug for RocksDB {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RocksDB(..)")
    }
}

impl fmt::Debug for RocksDBSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RocksDBSnapshot(..)")
    }
}

impl fmt::Debug for RocksDBTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RocksDBTransaction(..)")
    }
}

#[test]
fn test_rocksdb() {
    use tempdir::TempDir;

    let dir = TempDir::new("xxxxxxxx").unwrap();
    let mut opts = RocksDBOptions::default();
    opts.create_if_missing(true);
    let db = RocksDB::open(dir.path(), opts).unwrap();
    let fork = db.fork();

    assert!(!fork.contains("a", b"a"));
    fork.put("a", b"a", b"a");
    assert!(fork.contains("a", b"a"));

    let snapshot = db.snapshot();
    assert!(!snapshot.contains("a", b"a"));
    fork.commit();
    assert!(!snapshot.contains("a", b"a"));
    let snapshot = db.snapshot();
    assert!(snapshot.contains("a", b"a"));
}

#[test]
fn test_rocksdb_clean() {
    use tempdir::TempDir;

    let dir = TempDir::new("xxxxxxxx").unwrap();
    let mut opts = RocksDBOptions::default();
    opts.create_if_missing(true);
    let db = RocksDB::open(dir.path(), opts).unwrap();
    let fork = db.fork();

    fork.clear("a");
    assert!(!fork.contains("a", b"a"));
    fork.put("a", b"a", b"a");
    fork.put("a", b"b", b"b");
    assert!(fork.contains("a", b"a"));
    fork.commit();
    fork.clear("a");
    assert!(!fork.contains("a", b"a"));
    assert!(!fork.contains("a", b"b"));
}

#[test]
fn test_rocksdb_check_saved_data() {
    use tempdir::TempDir;

    let dir = TempDir::new("xxxxxxxx").unwrap();
    {
        let mut opts = RocksDBOptions::default();
        opts.create_if_missing(true);
        let db = RocksDB::open(dir.path(), opts).unwrap();
        let fork = db.fork();

        assert!(!fork.contains("a", b"a"));
        fork.put("a", b"a", b"a");
        fork.put("a", b"b", b"b");
        assert!(fork.contains("a", b"a"));
        fork.commit();
    }
    {
        let opts = RocksDBOptions::default();
        let db = match RocksDB::open(dir.path(), opts) {
            Ok(db) => db,
            Err(e) => panic!("Error while opening db: {}", e),
        };
        let fork = db.fork();
        assert!(fork.contains("a", b"a"));
        assert!(fork.contains("a", b"b"));
    }
}
