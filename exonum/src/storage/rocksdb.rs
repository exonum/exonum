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

#![allow(unsafe_code)]

//! An implementation of `RocksDB` database.

pub use rocksdb::{BlockBasedOptions as RocksBlockOptions, WriteOptions as RocksDBWriteOptions};

use rocksdb::{self, utils::get_cf_names, DBIterator, Options as RocksDbOptions, WriteBatch};

use std::{error::Error, fmt, iter::Peekable, mem, path::Path, sync::Arc};

use storage::{self, db::Change, Database, DbOptions, Iter, Iterator, Patch, Snapshot};

impl From<rocksdb::Error> for storage::Error {
    fn from(err: rocksdb::Error) -> Self {
        Self::new(err.description())
    }
}

/// Database implementation on top of [`RocksDB`](https://rocksdb.org)
/// backend.
///
/// `RocksDB` is an embedded database for key-value data, which is optimized for fast storage.
/// This structure is required to potentially adapt the interface to
/// use different databases.
pub struct RocksDB {
    db: Arc<rocksdb::DB>,
}

impl DbOptions {
    fn to_rocksdb(&self) -> RocksDbOptions {
        let mut defaults = RocksDbOptions::default();
        defaults.create_if_missing(self.create_if_missing);
        defaults.set_max_open_files(self.max_open_files.unwrap_or(-1));
        defaults
    }
}

/// A snapshot of a `RocksDB`.
pub struct RocksDBSnapshot {
    snapshot: rocksdb::Snapshot<'static>,
    db: Arc<rocksdb::DB>,
}

/// An iterator over the entries of a `RocksDB`.
struct RocksDBIterator {
    iter: Peekable<DBIterator>,
    key: Option<Box<[u8]>>,
    value: Option<Box<[u8]>>,
}

impl RocksDB {
    /// Opens a database stored at the specified path with the specified options.
    ///
    /// If the database does not exist at the indicated path and the option
    /// `create_if_missing` is switched on in `DbOptions`, a new database will
    /// be created at the indicated path.
    pub fn open<P: AsRef<Path>>(path: P, options: &DbOptions) -> storage::Result<Self> {
        let db = {
            if let Ok(names) = get_cf_names(&path) {
                let cf_names = names.iter().map(|name| name.as_str()).collect::<Vec<_>>();
                rocksdb::DB::open_cf(&options.to_rocksdb(), path, cf_names.as_ref())?
            } else {
                rocksdb::DB::open(&options.to_rocksdb(), path)?
            }
        };
        Ok(Self { db: Arc::new(db) })
    }

    fn do_merge(&self, patch: Patch, w_opts: &RocksDBWriteOptions) -> storage::Result<()> {
        let mut batch = WriteBatch::default();
        for (cf_name, changes) in patch {
            let cf = match self.db.cf_handle(&cf_name) {
                Some(cf) => cf,
                None => self
                    .db
                    .create_cf(&cf_name, &DbOptions::default().to_rocksdb())
                    .unwrap(),
            };
            for (key, change) in changes {
                match change {
                    Change::Put(ref value) => batch.put_cf(cf, key.as_ref(), value)?,
                    Change::Delete => batch.delete_cf(cf, &key)?,
                }
            }
        }
        self.db.write_opt(batch, w_opts).map_err(Into::into)
    }
}

impl Database for RocksDB {
    fn snapshot(&self) -> Box<dyn Snapshot> {
        Box::new(RocksDBSnapshot {
            snapshot: unsafe { mem::transmute(self.db.snapshot()) },
            db: Arc::clone(&self.db),
        })
    }

    fn merge(&self, patch: Patch) -> storage::Result<()> {
        let w_opts = RocksDBWriteOptions::default();
        self.do_merge(patch, &w_opts)
    }

    fn merge_sync(&self, patch: Patch) -> storage::Result<()> {
        let mut w_opts = RocksDBWriteOptions::default();
        w_opts.set_sync(true);
        self.do_merge(patch, &w_opts)
    }
}

impl Snapshot for RocksDBSnapshot {
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(cf) = self.db.cf_handle(name) {
            match self.snapshot.get_cf(cf, key) {
                Ok(value) => value.map(|v| v.to_vec()),
                Err(e) => panic!(e),
            }
        } else {
            None
        }
    }

    fn iter<'a>(&'a self, name: &str, from: &[u8]) -> Iter<'a> {
        use rocksdb::{Direction, IteratorMode};
        let iter = match self.db.cf_handle(name) {
            Some(cf) => self
                .snapshot
                .iterator_cf(cf, IteratorMode::From(from, Direction::Forward))
                .unwrap(),
            None => self.snapshot.iterator(IteratorMode::Start),
        };
        Box::new(RocksDBIterator {
            iter: iter.peekable(),
            key: None,
            value: None,
        })
    }
}

impl Iterator for RocksDBIterator {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        if let Some((key, value)) = self.iter.next() {
            self.key = Some(key);
            self.value = Some(value);
            Some((self.key.as_ref().unwrap(), self.value.as_ref().unwrap()))
        } else {
            None
        }
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        if let Some(&(ref key, ref value)) = self.iter.peek() {
            Some((key, value))
        } else {
            None
        }
    }
}

impl From<RocksDB> for Arc<dyn Database> {
    fn from(db: RocksDB) -> Self {
        Self::from(Box::new(db) as Box<dyn Database>)
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
