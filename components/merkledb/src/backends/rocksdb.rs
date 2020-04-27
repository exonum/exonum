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

//! An implementation of `RocksDB` database.

pub use rocksdb::{BlockBasedOptions as RocksBlockOptions, WriteOptions as RocksDBWriteOptions};

use crossbeam::sync::{ShardedLock, ShardedLockReadGuard};
use ctor::{ctor, dtor};
use rocksdb::{
    self, checkpoint::Checkpoint, ColumnFamily, DBIterator, Options as RocksDbOptions, WriteBatch,
};
use smallvec::SmallVec;

use std::{
    fmt,
    iter::Peekable,
    mem, ops,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::{
    db::{check_database, Change},
    Database, DbOptions, Iter, Iterator, Patch, ResolvedAddress, Snapshot,
};

/// Size of a byte representation of an index ID, which is used to prefix index keys
/// in a column family.
const ID_SIZE: usize = mem::size_of::<u64>();

/// Flag indicating that the main thread has exited.
#[ctor]
static FINISHED: AtomicBool = AtomicBool::new(false);

#[dtor]
fn finished() {
    FINISHED.store(true, Ordering::Release);
}

/// Container that does not drop its contents if the process is being shut down.
#[derive(Debug)]
pub(super) struct NoDropOnShutdown<T> {
    inner: mem::ManuallyDrop<T>,
}

impl<T> NoDropOnShutdown<T> {
    fn new(inner: T) -> Self {
        Self {
            inner: mem::ManuallyDrop::new(inner),
        }
    }
}

impl<T> Drop for NoDropOnShutdown<T> {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if !FINISHED.load(Ordering::Acquire) {
            unsafe {
                // SAFETY: `self.inner` is not used afterwards.
                mem::ManuallyDrop::drop(&mut self.inner);
            }
        }
    }
}

impl<T> ops::Deref for NoDropOnShutdown<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl<T> ops::DerefMut for NoDropOnShutdown<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.inner
    }
}

type InnerDB = NoDropOnShutdown<rocksdb::DB>;

/// Database implementation on top of [`RocksDB`](https://rocksdb.org)
/// backend.
///
/// `RocksDB` is an embedded database for key-value data, which is optimized for fast storage.
/// This structure is required to potentially adapt the interface to
/// use different databases.
pub struct RocksDB {
    db: Arc<ShardedLock<InnerDB>>,
    options: DbOptions,
}

impl From<DbOptions> for RocksDbOptions {
    fn from(opts: DbOptions) -> Self {
        Self::from(&opts)
    }
}

impl From<&DbOptions> for RocksDbOptions {
    fn from(opts: &DbOptions) -> Self {
        let mut defaults = Self::default();
        defaults.create_if_missing(opts.create_if_missing);
        defaults.set_compression_type(opts.compression_type.into());
        defaults.set_max_open_files(opts.max_open_files.unwrap_or(-1));
        defaults
    }
}

/// A snapshot of a `RocksDB`.
pub struct RocksDBSnapshot {
    snapshot: rocksdb::Snapshot<'static>,
    db: Arc<ShardedLock<InnerDB>>,
}

/// An iterator over the entries of a `RocksDB`.
struct RocksDBIterator<'a> {
    iter: Peekable<DBIterator<'a>>,
    key: Option<Box<[u8]>>,
    value: Option<Box<[u8]>>,
    prefix: Option<[u8; ID_SIZE]>,
    ended: bool,
}

impl RocksDB {
    /// Opens a database stored at the specified path with the specified options.
    ///
    /// If the database does not exist at the indicated path and the option
    /// `create_if_missing` is switched on in `DbOptions`, a new database will
    /// be created at the indicated path.
    pub fn open<P: AsRef<Path>>(path: P, options: &DbOptions) -> crate::Result<Self> {
        let inner = {
            if let Ok(names) = rocksdb::DB::list_cf(&RocksDbOptions::default(), &path) {
                let cf_names = names.iter().map(String::as_str).collect::<Vec<_>>();
                rocksdb::DB::open_cf(&options.into(), path, cf_names)?
            } else {
                rocksdb::DB::open(&options.into(), path)?
            }
        };
        let mut db = Self {
            db: Arc::new(ShardedLock::new(NoDropOnShutdown::new(inner))),
            options: *options,
        };
        check_database(&mut db)?;
        Ok(db)
    }

    /// Creates checkpoint of this database in the given directory. See [`RocksDB` docs] for
    /// details.
    ///
    /// Successfully created checkpoint can be opened using `RocksDB::open`.
    ///
    /// [`RocksDB` docs]: https://github.com/facebook/rocksdb/wiki/Checkpoints
    pub fn create_checkpoint<T: AsRef<Path>>(&self, path: T) -> crate::Result<()> {
        let checkpoint = Checkpoint::new(&*self.get_lock_guard())?;
        checkpoint.create_checkpoint(path)?;
        Ok(())
    }

    fn cf_exists(&self, cf_name: &str) -> bool {
        self.get_lock_guard().cf_handle(cf_name).is_some()
    }

    fn create_cf(&self, cf_name: &str) -> crate::Result<()> {
        self.db
            .write()
            .expect("Couldn't get write lock to DB")
            .create_cf(cf_name, &self.options.into())
            .map_err(Into::into)
    }

    pub(super) fn get_lock_guard(&self) -> ShardedLockReadGuard<'_, InnerDB> {
        self.db.read().expect("Couldn't get read lock to DB")
    }

    /// Clears the column family completely, removing all keys from it.
    pub(super) fn clear_column_family(
        &self,
        batch: &mut WriteBatch,
        cf: &ColumnFamily,
    ) -> crate::Result<()> {
        /// Some lexicographically large key.
        const LARGER_KEY: &[u8] = &[u8::max_value(); 1_024];

        let db_reader = self.get_lock_guard();
        let mut iter = db_reader.raw_iterator_cf(cf)?;
        iter.seek_to_last();
        if iter.valid() {
            if let Some(key) = iter.key() {
                // For some reason, removing a range to a very large key is
                // significantly faster than removing the exact range.
                // This is specific to the debug mode, but since `TemporaryDB`
                // is mostly used for testing, this optimization leads to practical
                // performance improvement.
                if key.len() < LARGER_KEY.len() {
                    batch.delete_range_cf::<&[u8]>(cf, &[], LARGER_KEY)?;
                } else {
                    batch.delete_range_cf::<&[u8]>(cf, &[], key)?;
                    batch.delete_cf(cf, &key)?;
                }
            }
        }
        Ok(())
    }

    fn do_merge(&self, patch: Patch, w_opts: &RocksDBWriteOptions) -> crate::Result<()> {
        let mut batch = WriteBatch::default();
        for (resolved, changes) in patch.into_changes() {
            if !self.cf_exists(&resolved.name) {
                self.create_cf(&resolved.name)?;
            }

            let db_reader = self.get_lock_guard();
            let cf = db_reader.cf_handle(&resolved.name).unwrap();

            if changes.is_cleared() {
                self.clear_prefix(&mut batch, cf, &resolved)?;
            }

            if let Some(id_bytes) = resolved.id_to_bytes() {
                // Write changes to the column family with each key prefixed by the ID of the
                // resolved address.

                // We assume that typical key sizes are less than `1_024 - ID_SIZE = 1_016` bytes,
                // so that they fit into stack.
                let mut buffer: SmallVec<[u8; 1_024]> = SmallVec::new();
                buffer.extend_from_slice(&id_bytes);

                for (key, change) in changes.into_data() {
                    buffer.truncate(ID_SIZE);
                    buffer.extend_from_slice(&key);
                    match change {
                        Change::Put(ref value) => batch.put_cf(cf, &buffer, value)?,
                        Change::Delete => batch.delete_cf(cf, &buffer)?,
                    }
                }
            } else {
                // Write changes to the column family as-is.
                for (key, change) in changes.into_data() {
                    match change {
                        Change::Put(ref value) => batch.put_cf(cf, &key, value)?,
                        Change::Delete => batch.delete_cf(cf, &key)?,
                    }
                }
            }
        }

        self.get_lock_guard()
            .write_opt(batch, w_opts)
            .map_err(Into::into)
    }

    /// Removes all keys with the specified prefix from a column family.
    fn clear_prefix(
        &self,
        batch: &mut WriteBatch,
        cf: &ColumnFamily,
        resolved: &ResolvedAddress,
    ) -> crate::Result<()> {
        if let Some(id_bytes) = resolved.id_to_bytes() {
            let next_bytes = next_id_bytes(id_bytes);
            batch
                .delete_range_cf(cf, id_bytes, next_bytes)
                .map_err(Into::into)
        } else {
            self.clear_column_family(batch, cf)
        }
    }

    #[allow(unsafe_code)]
    pub(super) fn rocksdb_snapshot(&self) -> RocksDBSnapshot {
        RocksDBSnapshot {
            // SAFETY:
            // The snapshot carries an `Arc` to the database to make sure that database
            // is not dropped before the snapshot. Additionally, the pointer to `rocksdb::DB`
            // is stable within `Arc<ShardedLock<rocksdb::DB>>` and its part used in dropping
            // the snapshot (`*mut ffi::rocksdb_t`) is never changed, i.e., not affected
            // by potential incoherence if the `ShardedLock` is being concurrently written to.
            // FIXME: Investigate changing `rocksdb::Snapshot` / `DB` to remove `unsafe` (ECR-4273).
            snapshot: unsafe { mem::transmute(self.get_lock_guard().snapshot()) },
            db: Arc::clone(&self.db),
        }
    }
}

impl RocksDBSnapshot {
    fn get_lock_guard(&self) -> ShardedLockReadGuard<'_, InnerDB> {
        self.db.read().expect("Couldn't get read lock to DB")
    }

    fn rocksdb_iter(&self, name: &ResolvedAddress, from: &[u8]) -> RocksDBIterator<'_> {
        use rocksdb::{Direction, IteratorMode};

        let from = name.keyed(from);
        let iter = match self.get_lock_guard().cf_handle(&name.name) {
            Some(cf) => self
                .snapshot
                .iterator_cf(cf, IteratorMode::From(from.as_ref(), Direction::Forward))
                .unwrap(),
            None => self.snapshot.iterator(IteratorMode::Start),
        };
        RocksDBIterator {
            iter: iter.peekable(),
            prefix: name.id_to_bytes(),
            key: None,
            value: None,
            ended: false,
        }
    }
}

impl Database for RocksDB {
    fn snapshot(&self) -> Box<dyn Snapshot> {
        Box::new(self.rocksdb_snapshot())
    }

    fn merge(&self, patch: Patch) -> crate::Result<()> {
        let w_opts = RocksDBWriteOptions::default();
        self.do_merge(patch, &w_opts)
    }

    fn merge_sync(&self, patch: Patch) -> crate::Result<()> {
        let mut w_opts = RocksDBWriteOptions::default();
        w_opts.set_sync(true);
        self.do_merge(patch, &w_opts)
    }
}

impl Snapshot for RocksDBSnapshot {
    fn get(&self, resolved_addr: &ResolvedAddress, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(cf) = self.get_lock_guard().cf_handle(&resolved_addr.name) {
            match self.snapshot.get_cf(cf, resolved_addr.keyed(key)) {
                Ok(value) => value.map(|v| v.to_vec()),
                Err(e) => panic!(e),
            }
        } else {
            None
        }
    }

    fn iter(&self, name: &ResolvedAddress, from: &[u8]) -> Iter<'_> {
        Box::new(self.rocksdb_iter(name, from))
    }
}

impl<'a> Iterator for RocksDBIterator<'a> {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        if self.ended {
            return None;
        }

        let (key, value) = self.iter.next()?;
        if let Some(ref prefix) = self.prefix {
            if &key[..ID_SIZE] != prefix {
                self.ended = true;
                return None;
            }
        }

        self.key = Some(key);
        let key = if self.prefix.is_some() {
            &self.key.as_ref()?[ID_SIZE..]
        } else {
            &self.key.as_ref()?[..]
        };
        self.value = Some(value);
        Some((key, self.value.as_ref()?))
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        if self.ended {
            return None;
        }

        let (key, value) = self.iter.peek()?;
        let key = if let Some(prefix) = self.prefix {
            if key[..ID_SIZE] != prefix {
                self.ended = true;
                return None;
            }
            &key[ID_SIZE..]
        } else {
            &key[..]
        };
        Some((key, &value[..]))
    }
}

impl From<RocksDB> for Arc<dyn Database> {
    fn from(db: RocksDB) -> Self {
        Self::from(Box::new(db) as Box<dyn Database>)
    }
}

impl fmt::Debug for RocksDB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RocksDB").finish()
    }
}

impl fmt::Debug for RocksDBSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RocksDBSnapshot").finish()
    }
}

/// Generates the sequence of bytes lexicographically following the provided one. Assumes that
/// the provided sequence is less than `[u8::max_value(); ID_SIZE]`.
fn next_id_bytes(id_bytes: [u8; ID_SIZE]) -> [u8; ID_SIZE] {
    let mut next_id_bytes = id_bytes;
    for byte in next_id_bytes.iter_mut().rev() {
        if *byte == u8::max_value() {
            *byte = 0;
        } else {
            *byte += 1;
            break;
        }
    }
    next_id_bytes
}

#[test]
fn test_next_id_bytes() {
    assert_eq!(
        next_id_bytes([1, 0, 0, 0, 0, 0, 0, 0]),
        [1, 0, 0, 0, 0, 0, 0, 1]
    );
    assert_eq!(
        next_id_bytes([1, 2, 3, 4, 5, 6, 7, 8]),
        [1, 2, 3, 4, 5, 6, 7, 9]
    );
    assert_eq!(
        next_id_bytes([1, 0, 0, 0, 0, 0, 0, 254]),
        [1, 0, 0, 0, 0, 0, 0, 255]
    );
    assert_eq!(
        next_id_bytes([1, 0, 0, 0, 0, 0, 41, 255]),
        [1, 0, 0, 0, 0, 0, 42, 0]
    );
    assert_eq!(
        next_id_bytes([1, 2, 3, 4, 5, 255, 255, 255]),
        [1, 2, 3, 4, 6, 0, 0, 0]
    );
}

/// To reproduce UB in the test, it should run in isolation. The UB may vanish on consecutive
/// launches of the test.
///
/// The UB is most often realized as `pthread lock: Invalid argument`, but may manifest with
/// other errors.
#[test]
fn concurrency_is_hard() {
    use std::{thread, time::Duration};

    let mut options = DbOptions::default();
    options.create_if_missing = true;
    let options = &options;

    let names = rocksdb::DB::list_cf(&options.into(), "/tmp/rocksdb").unwrap_or_default();
    let cf_names = names.iter().map(String::as_str).collect::<Vec<_>>();
    let db = rocksdb::DB::open_cf(&options.into(), "/tmp/rocksdb", cf_names).unwrap();
    let db = NoDropOnShutdown::new(db);

    let signal = Arc::new(AtomicBool::new(true));
    let signal_ = Arc::clone(&signal);

    thread::spawn(move || {
        for _ in 0..200 {
            thread::sleep(Duration::from_millis(1));
            if !signal_.load(Ordering::Acquire) {
                drop(db);
                return;
            }
        }
    });

    thread::sleep(Duration::from_millis(20));
    signal.store(false, Ordering::Release);
}
