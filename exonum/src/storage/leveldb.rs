use profiler;

use leveldb::database::Database as _LevelDB;
use leveldb::iterator::{Iterator as _Iterator, Iterable};
use leveldb::error::Error as _Error;
use leveldb::database::snapshots::Snapshot as _Snapshot;
use leveldb::options::{WriteOptions, ReadOptions};
use leveldb::database::batch::Writebatch;
use leveldb::batch::Batch;
use leveldb::snapshots::Snapshots;

use std::fs;
use std::io;
use std::mem;
use std::path::Path;
use std::error;
use std::sync::Arc;

pub use leveldb::options::Options as LevelDBOptions;
pub use leveldb::database::cache::Cache as LevelDBCache;

use super::{Database, Iterator, Iter, Snapshot, Error, Patch, Change, Result};

const LEVELDB_READ_OPTIONS: ReadOptions<'static> = ReadOptions {
    verify_checksums: false,
    fill_cache: true,
    snapshot: None,
};

const LEVELDB_WRITE_OPTIONS: WriteOptions = WriteOptions { sync: false };

#[derive(Clone)]
pub struct LevelDB {
    db: Arc<_LevelDB>,
}

struct LevelDBSnapshot {
    _db: Arc<_LevelDB>,
    snapshot: _Snapshot<'static>,
}

struct LevelDBIterator<'a> {
    iter: _Iterator<'a>,
}

impl From<_Error> for Error {
    fn from(err: _Error) -> Self {
        Error::new(error::Error::description(&err))
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::new(error::Error::description(&err))
    }
}

impl LevelDB {
    // TODO: configurate LRU cache
    pub fn open<P: AsRef<Path>>(path: P, options: LevelDBOptions) -> Result<LevelDB> {
        if options.create_if_missing {
            fs::create_dir_all(path.as_ref())?;
        }
        let database = _LevelDB::open(path.as_ref(), options)?;
        Ok(LevelDB { db: Arc::new(database) })
    }
}

impl Database for LevelDB {
    fn clone(&self) -> Box<Database> {
        Box::new(Clone::clone(self))
    }

    fn snapshot(&self) -> Box<Snapshot> {
        let _p = profiler::ProfilerSpan::new("LevelDB::snapshot");
        Box::new(LevelDBSnapshot {
                     _db: self.db.clone(),
                     snapshot: unsafe { mem::transmute(self.db.snapshot()) },
                 })
    }

    fn merge(&mut self, patch: Patch) -> Result<()> {
        let _p = profiler::ProfilerSpan::new("LevelDB::merge");
        let mut batch = Writebatch::new();
        for (key, change) in patch {
            match change {
                Change::Put(ref v) => batch.put(key, v),
                Change::Delete => batch.delete(key),
            }
        }
        self.db
            .write(LEVELDB_WRITE_OPTIONS, &batch)
            .map_err(Into::into)
    }
}

impl Snapshot for LevelDBSnapshot {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let _p = profiler::ProfilerSpan::new("LevelDBSnapshot::get");
        match self.snapshot.get(LEVELDB_READ_OPTIONS, key) {
            Ok(value) => value,
            Err(err) => panic!(err),
        }
    }

    fn iter<'a>(&'a self, from: &[u8]) -> Iter<'a> {
        let _p = profiler::ProfilerSpan::new("LevelDBSnapshot::iter");
        let mut iter = self.snapshot.iter(LEVELDB_READ_OPTIONS);
        iter.seek(from);
        Box::new(LevelDBIterator { iter: iter })
    }
}

impl<'a> Iterator for LevelDBIterator<'a> {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        let _p = profiler::ProfilerSpan::new("LevelDBIterator::next");
        self.iter.next()
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        let _p = profiler::ProfilerSpan::new("LevelDBIterator::peek");
        self.iter.peek()
    }
}

impl ::std::fmt::Debug for LevelDB {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "LevelDB(..)")
    }
}

impl ::std::fmt::Debug for LevelDBSnapshot {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "LevelDBSnapshot(..)")
    }
}
