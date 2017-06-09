use leveldb::database::Database as LevelDatabase;
use leveldb::iterator::{LevelDBIterator, Iterable};
use leveldb::error::Error as LevelDBError;
use leveldb::database::snapshots::Snapshot as LevelSnapshot;
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

use super::{Database, Iter, Snapshot, Error, Patch, Change, Result};

const LEVELDB_READ_OPTIONS: ReadOptions<'static> = ReadOptions {
    verify_checksums: false,
    fill_cache: true,
    snapshot: None,
};

const LEVELDB_WRITE_OPTIONS: WriteOptions = WriteOptions { sync: false };

#[derive(Clone)]
pub struct LevelDB {
    db: Arc<LevelDatabase>,
}

struct LevelDBSnapshot {
    _db: Arc<LevelDatabase>,
    snapshot: LevelSnapshot<'static>,
}

impl From<LevelDBError> for Error {
    fn from(err: LevelDBError) -> Self {
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
    pub fn open(path: &Path, options: LevelDBOptions) -> Result<LevelDB> {
        if options.create_if_missing {
            fs::create_dir_all(path)?;
        }
        let database = LevelDatabase::open(path, options)?;
        Ok(LevelDB { db: Arc::new(database) })
    }
}

impl Database for LevelDB {
    fn clone(&self) -> Box<Database> {
        Box::new(Clone::clone(self))
    }

    fn snapshot(&self) -> Box<Snapshot> {
        Box::new(LevelDBSnapshot {
            _db: self.db.clone(),
            snapshot: unsafe { mem::transmute(self.db.snapshot()) }
        })
    }

    fn merge(&mut self, patch: Patch) -> Result<()> {
        let mut batch = Writebatch::new();
        for (key, change) in patch {
            match change {
                Change::Put(ref v) => batch.put(key, v),
                Change::Delete => batch.delete(key)
            }
        }
        self.db.write(LEVELDB_WRITE_OPTIONS, &batch).map_err(Into::into)
    }
}

impl Snapshot for LevelDBSnapshot {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self.snapshot.get(LEVELDB_READ_OPTIONS, key) {
            Ok(value) => value,
            Err(err) => panic!(err)
        }
    }

    fn iter<'a>(&'a self, from: &[u8]) -> Iter<'a> {
        let iter = self.snapshot.iter(LEVELDB_READ_OPTIONS);
        iter.seek(from);
        Box::new(iter)
    }
}
