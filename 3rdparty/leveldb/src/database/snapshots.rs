//! leveldb snapshots
//!
//! Snapshots give you a reference to the database at a certain
//! point in time and won't change while you work with them.
use leveldb_sys::{leveldb_t, leveldb_snapshot_t};
use leveldb_sys::{leveldb_release_snapshot, leveldb_create_snapshot};

use database::Database;
use database::kv::KV;

use database::error::Error;
use database::options::ReadOptions;
use database::iterator::{Iterable, Iterator, KeyIterator, ValueIterator};

#[allow(missing_docs)]
struct RawSnapshot {
    db_ptr: *mut leveldb_t,
    ptr: *mut leveldb_snapshot_t,
}

impl Drop for RawSnapshot {
    fn drop(&mut self) {
        unsafe { leveldb_release_snapshot(self.db_ptr, self.ptr) };
    }
}

/// A database snapshot
///
/// Represents a database at a certain point in time,
/// and allows for all read operations (get and iteration).
pub struct Snapshot<'a> {
    raw: RawSnapshot,
    database: &'a Database,
}

/// Structs implementing the Snapshots trait can be
/// snapshotted.
pub trait Snapshots {
    /// Creates a snapshot and returns a struct
    /// representing it.
    fn snapshot(&self) -> Snapshot;
}

impl Snapshots for Database {
    fn snapshot(&self) -> Snapshot {
        let db_ptr = self.database.ptr;
        let snap = unsafe { leveldb_create_snapshot(db_ptr) };

        let raw = RawSnapshot {
            db_ptr: db_ptr,
            ptr: snap,
        };
        Snapshot {
            raw: raw,
            database: self,
        }
    }
}

impl<'a> Snapshot<'a> {
    /// fetches a key from the database
    ///
    /// Inserts this snapshot into ReadOptions before reading
    pub fn get<K: AsRef<[u8]>>(&'a self,
                               mut options: ReadOptions<'a>,
                               key: K)
                               -> Result<Option<Vec<u8>>, Error> {
        options.snapshot = Some(self);
        self.database.get(options, key.as_ref())
    }

    #[inline]
    #[allow(missing_docs)]
    pub fn raw_ptr(&self) -> *mut leveldb_snapshot_t {
        self.raw.ptr
    }
}

impl<'a> Iterable<'a> for Snapshot<'a> {
    fn iter(&'a self, mut options: ReadOptions<'a>) -> Iterator {
        options.snapshot = Some(self);
        self.database.iter(options)
    }
    fn keys_iter(&'a self, mut options: ReadOptions<'a>) -> KeyIterator {
        options.snapshot = Some(self);
        self.database.keys_iter(options)
    }
    fn value_iter(&'a self, mut options: ReadOptions<'a>) -> ValueIterator {
        options.snapshot = Some(self);
        self.database.value_iter(options)
    }
}
