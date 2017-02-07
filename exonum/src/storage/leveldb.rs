use std::fs;
use std::io;
use std::mem;
use std::path::Path;
use std::error;
use std::sync::Arc;
use std::cell::RefCell;
use std::collections::Bound::{Included, Unbounded};
// use std::iter::Iterator;

use leveldb::database::Database as LevelDatabase;
use leveldb::error::Error as LevelError;
use leveldb::database::iterator::Iterable as LevelIterable;
use leveldb::database::snapshots::Snapshot as LevelSnapshot;
use leveldb::options::{Options, WriteOptions, ReadOptions};
use leveldb::database::kv::KV;
use leveldb::database::batch::Writebatch;
use leveldb::batch::Batch;
use leveldb::snapshots::Snapshots;
// use leveldb::database::iterator::Iterator as LevelIterator;
use leveldb::iterator::LevelDBIterator;

use super::{Map, Database, Error, Patch, Change, Fork};
// use super::{Iterable, Seekable}

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

impl LevelDBView {
    pub fn new(from: &LevelDB) -> LevelDBView {
        LevelDBView {
            _db: from.db.clone(),
            snap: unsafe { mem::transmute(from.db.snapshot()) },
            changes: RefCell::default(),
        }
    }
}

// FIXME: remove this implementation
impl Map<[u8], Vec<u8>> for LevelDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        self.db
            .get(LEVELDB_READ_OPTIONS, key)
            .map_err(Into::into)
    }

    fn put(&self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
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


impl Map<[u8], Vec<u8>> for LevelDBView {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
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
        self.changes.borrow_mut().insert(key.to_vec(), Change::Put(value));
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), Error> {
        self.changes.borrow_mut().insert(key.to_vec(), Change::Delete);
        Ok(())
    }

    fn find_key(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        // TODO merge with the same function in memorydb
        let out = {
            let map = self.changes.borrow();
            let mut it = map.range::<[u8], [u8]>(Included(key), Unbounded);
            it.next().map(|x| x.0.to_vec())
        };
        if out.is_none() {
            let it = self.snap.keys_iter(LEVELDB_READ_OPTIONS);
            it.seek(key.to_vec());
            if it.valid() {
                let key = it.key();
                return Ok(Some(key.to_vec()));
            }
            Ok(None)
        } else {
            Ok(out)
        }
    }
}

impl Fork for LevelDBView {
    fn changes(&self) -> Patch {
        self.changes.borrow().clone()
    }
    fn merge(&self, patch: &Patch) {
        let iter = patch.into_iter().map(|(k, v)| (k.clone(), v.clone()));
        self.changes.borrow_mut().extend(iter);
    }
}

impl Database for LevelDB {
    type Fork = LevelDBView;

    fn fork(&self) -> Self::Fork {
        LevelDBView::new(self)
    }

    // fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    //     self.db
    //         .get(LEVELDB_READ_OPTIONS, key)
    //         .map_err(Into::into)
    // }

    // fn put(&self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
    //     let result = self.db.put(LEVELDB_WRITE_OPTIONS, key, &value);
    //     result.map_err(Into::into)
    // }

    // fn delete(&self, key: &[u8]) -> Result<(), Error> {
    //     let result = self.db.delete(LEVELDB_WRITE_OPTIONS, key);
    //     result.map_err(Into::into)
    // }

    fn merge(&self, patch: &Patch) -> Result<(), Error> {
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

// pub struct DatabaseIterator<'a> {
//     iter: LevelIterator<'a, BinaryKey>
// }

// impl<'a> Iterator for DatabaseIterator<'a> {
//     type Item = (Vec<u8>, Vec<u8>);

//     fn next(&mut self) -> Option<Self::Item> {
//         let item = self.iter.next();
//         item.map(|x| ((x.0).0, x.1))
//     }
// }

// impl<'a> Iterable for &'a LevelDB {
//     type Iter = DatabaseIterator<'a>;

//     fn iter(self) -> Self::Iter {
//         DatabaseIterator {
//             iter: self.db.iter(LEVELDB_READ_OPTIONS)
//         }
//     }
// }

// impl<'a> Seekable<'a> for DatabaseIterator<'a> {
//     type Key = Vec<u8>;
//     type Item = (Vec<u8>, Vec<u8>);

//     // TODO I am not sure that optimizer will remove memory allocation here
//     fn seek(&mut self, key: &Self::Key) -> Option<Self::Item> {
//         let db_key = BinaryKey(key.to_vec());
//         self.iter.seek(&db_key);
//         Some((self.iter.key().0, self.iter.value()))
//     }
// }
