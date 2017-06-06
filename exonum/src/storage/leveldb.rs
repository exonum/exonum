use leveldb::database::Database as LevelDatabase;
use leveldb::error::Error as LevelError;
use leveldb::database::iterator::Iterable as LevelIterable;
use leveldb::database::snapshots::Snapshot as LevelSnapshot;
use leveldb::options::{Options, WriteOptions, ReadOptions};
use leveldb::database::kv::KV;
use leveldb::database::batch::Writebatch;
use leveldb::batch::Batch;
use leveldb::snapshots::Snapshots;
use leveldb::iterator::LevelDBIterator;

use std::fs;
use std::io;
use std::mem;
use std::fmt;
use std::path::Path;
use std::error;
use std::sync::Arc;
use std::cell::RefCell;
use std::cmp::Ordering;

use profiler;

use super::{Map, Database, Error, Patch, Change, Fork};

#[derive(Clone)]
pub struct LevelDB {
    db: Arc<LevelDatabase>,
}

pub struct LevelDBView {
    _db: Arc<LevelDatabase>,
    snap: LevelSnapshot<'static>,
    changes: RefCell<Patch>,
}

const LEVELDB_READ_OPTIONS: ReadOptions<'static> = ReadOptions {
    verify_checksums: false,
    fill_cache: true,
    snapshot: None,
};
const LEVELDB_WRITE_OPTIONS: WriteOptions = WriteOptions { sync: false };

impl From<LevelError> for Error {
    fn from(err: LevelError) -> Self {
        Error::new(error::Error::description(&err))
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::new(error::Error::description(&err))
    }
}

impl LevelDB {
    pub fn new(path: &Path, options: Options) -> Result<LevelDB, Error> {
        if options.create_if_missing {
            fs::create_dir_all(path)?;
        }
        let database = LevelDatabase::open(path, options)?;
        Ok(LevelDB { db: Arc::new(database) })
    }
}

impl Map<[u8], Vec<u8>> for LevelDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let _p = profiler::ProfilerSpan::new("LevelDB::get");
        self.db
            .get(LEVELDB_READ_OPTIONS, key)
            .map_err(Into::into)
    }

    fn put(&self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
        let _p = profiler::ProfilerSpan::new("LevelDB::put");
        let result = self.db.put(LEVELDB_WRITE_OPTIONS, key, &value);
        result.map_err(Into::into)
    }

    fn delete(&self, key: &[u8]) -> Result<(), Error> {
        let result = self.db.delete(LEVELDB_WRITE_OPTIONS, key);
        result.map_err(Into::into)
    }
    fn find_key(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let it = self.db.keys_iter(LEVELDB_READ_OPTIONS);
        it.seek(key);
        if it.valid() {
            let key = it.key();
            return Ok(Some(key.to_vec()));
        }
        Ok(None)
    }
}

impl LevelDBView {
    pub fn new(from: &LevelDB) -> LevelDBView {
        LevelDBView {
            _db: from.db.clone(),
            snap: unsafe { mem::transmute(from.db.snapshot()) },
            changes: RefCell::default(),
        }
    }
}

impl Map<[u8], Vec<u8>> for LevelDBView {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let mut _p = profiler::ProfilerSpan::new("LevelDBView::get");
        match self.changes.borrow().get(key) {
            Some(change) => {
                let v = match *change {
                    Change::Put(ref v) => Some(v.clone()),
                    Change::Delete => None,
                };
                Ok(v)
            }
            None => {
                self.snap
                    .get(LEVELDB_READ_OPTIONS, key)
                    .map_err(Into::into)
            }
        }
    }

    fn put(&self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
        let _p = profiler::ProfilerSpan::new("LevelDBView::put");
        self.changes.borrow_mut().insert(key.to_vec(), Change::Put(value));
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), Error> {
        self.changes.borrow_mut().insert(key.to_vec(), Change::Delete);
        Ok(())
    }

    fn find_key(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let map_changes = self.changes.borrow();
        let mut it_changes = map_changes.range(key.to_vec()..);
        let mut it_snapshot = self.snap.keys_iter(LEVELDB_READ_OPTIONS).from(key);

        let res: Option<Vec<u8>>;
        let least_put_key: Option<Vec<u8>> = it_changes.find(|entry| {
                match *entry.1 {
                    Change::Delete => false,
                    Change::Put(_) => true, 
                }
            })
            .map(|x| x.0.to_vec());

        loop {
            let first_snapshot: Option<&[u8]> = it_snapshot.next();
            match first_snapshot {
                Some(snap_key) => {
                    let change_for_key: Option<&Change> = map_changes.get(snap_key);
                    if let Some(&Change::Delete) = change_for_key {
                        continue;
                    } else {
                        let snap_key_vec = snap_key.to_vec();

                        if let Some(put_key) = least_put_key {
                            let cmp = snap_key_vec.cmp(&put_key);
                            if let Ordering::Greater = cmp {
                                res = Some(put_key);
                                break;
                            }
                        }
                        res = Some(snap_key_vec);
                        break;
                    }
                } 
                None => {
                    res = least_put_key;
                    break;
                }
            }
        }
        Ok(res)
    }
}

impl Fork for LevelDBView {
    fn changes(&self) -> Patch {
        self.changes.borrow().clone()
    }
    fn merge(&self, patch: &Patch) {
        let _p = profiler::ProfilerSpan::new("LevelDBView::merge");
        let iter = patch.into_iter().map(|(k, v)| (k.clone(), v.clone()));
        self.changes.borrow_mut().extend(iter);
    }
}

impl Database for LevelDB {
    type Fork = LevelDBView;

    fn fork(&self) -> Self::Fork {
        LevelDBView::new(self)
    }

    fn merge(&self, patch: &Patch) -> Result<(), Error> {
        let _p = profiler::ProfilerSpan::new("LevelDB::merge");
        let mut batch = Writebatch::new();
        for (key, change) in patch {
            match *change {
                Change::Put(ref v) => {
                    batch.put(key, v);
                }
                Change::Delete => {
                    batch.delete(key);
                }
            }
        }
        let write_opts = WriteOptions::new();
        let result = self.db.write(write_opts, &batch);
        result.map_err(Into::into)
    }
}

impl fmt::Debug for LevelDB {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("LevelDB { .. }")
    }
}

impl fmt::Debug for LevelDBView {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("LevelDBView { .. }")
    }
}