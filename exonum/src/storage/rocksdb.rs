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
use profiler;

use rocksdb::TransactionDB as _RocksDB;
use rocksdb::DBRawIterator;
use rocksdb::Snapshot as _Snapshot;
use rocksdb::Error as _Error;

use std::mem;
use std::sync::Arc;
use std::path::Path;
use std::fmt;
use std::error;

pub use rocksdb::Options as RocksDBOptions;
pub use rocksdb::BlockBasedOptions as RocksBlockOptions;
pub use rocksdb::{TransactionDBOptions, TransactionOptions, WriteOptions};

use super::{Database, Iterator, Iter, Snapshot, Error, Patch, Change, Result};

impl From<_Error> for Error {
    fn from(err: _Error) -> Self {
        Error::new(error::Error::description(&err))
    }
}

/// Database implementation on the top of `RocksDB` backend.
#[derive(Clone)]
pub struct RocksDB {
    db: Arc<_RocksDB>,
}

/// A snapshot of a `RocksDB`.
pub struct RocksDBSnapshot {
    _db: Arc<_RocksDB>,
    snapshot: _Snapshot<'static>,
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
		let txn_db_options = TransactionDBOptions::default();
        let database = _RocksDB::open(&options, &txn_db_options, path)?;
        Ok(RocksDB { db: Arc::new(database) })
    }
}

impl Database for RocksDB {
    fn clone(&self) -> Box<Database> {
        Box::new(Clone::clone(self))
    }

    fn snapshot(&self) -> Box<Snapshot> {
        let _p = profiler::ProfilerSpan::new("RocksDB::snapshot");
        Box::new(RocksDBSnapshot {
            _db: self.db.clone(),
            snapshot: unsafe { mem::transmute(self.db.snapshot()) },
        })
    }

    fn merge(&mut self, patch: Patch) -> Result<()> {
        let _p = profiler::ProfilerSpan::new("RocksDB::merge");
		let w_opts = WriteOptions::default();
		let txn_opts = TransactionOptions::default();
		let txn = self.db.transaction_begin(&w_opts, &txn_opts);
        for (key, change) in patch {
            match change {
                Change::Put(ref value) => txn.put(&key, value)?,
                Change::Delete => txn.delete(&key)?,
            }
        }
		txn.commit().map_err(Into::into)
    }
}

impl Snapshot for RocksDBSnapshot {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let _p = profiler::ProfilerSpan::new("RocksDBSnapshot::get");
        match self.snapshot.get(key) {
            Ok(value) => value.map(|v| v.to_vec()),
            Err(e) => panic!(e),
        }
    }

    fn iter<'a>(&'a self, from: &[u8]) -> Iter<'a> {
        let _p = profiler::ProfilerSpan::new("RocksDBSnapshot::iter");
        let mut iter = self.snapshot.raw_iterator();
        iter.seek(from);
        Box::new(RocksDBIterator {
            iter: iter,
            key: None,
            value: None,
        })
    }
}

impl<'a> Iterator for RocksDBIterator {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        let _p = profiler::ProfilerSpan::new("RocksDBIterator::next");
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
        let _p = profiler::ProfilerSpan::new("RocksDBIterator::peek");
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
