use std::path::Path;
use std::fmt::Debug;
// use std::iter::Iterator;

use db_key::Key;
use leveldb::database::Database as LevelDatabase;
use leveldb::error::Error as LevelError;
use leveldb::options::{Options, WriteOptions, ReadOptions};
use leveldb::database::kv::KV;
use leveldb::database::batch::Writebatch;
use leveldb::batch::Batch;
// use leveldb::database::iterator::Iterator as LevelIterator;
use leveldb::database::iterator::Iterable as LevelIterable;
use leveldb::iterator::LevelDBIterator;

use super::{Map, Database, Error, Patch, Change};
// use super::{Iterable, Seekable}

struct BinaryKey(Vec<u8>);

impl Key for BinaryKey {
    fn from_u8(key: &[u8]) -> BinaryKey {
        BinaryKey(key.to_vec())
    }

    fn as_slice<T, F: Fn(&[u8]) -> T>(&self, f: F) -> T {
        f(&self.0)
    }
}

pub struct LevelDB {
    db: LevelDatabase<BinaryKey>,
}

const LEVELDB_READ_OPTIONS: ReadOptions<'static, BinaryKey> = ReadOptions {
    verify_checksums: false,
    fill_cache: true,
    snapshot: None,
};
const LEVELDB_WRITE_OPTIONS: WriteOptions = WriteOptions { sync: false };

impl LevelDB {
    pub fn new(path: &Path, options: Options) -> Result<LevelDB, Error> {
        match LevelDatabase::open(path, options) {
            Ok(database) => Ok(LevelDB { db: database }),
            Err(e) => Err(Self::to_storage_error(e)),
        }
    }

    fn to_storage_error(err: LevelError) -> Error {
        Box::new(err) as Box<Debug>
    }
}

impl Map<[u8], Vec<u8>> for LevelDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        self.db
            .get(LEVELDB_READ_OPTIONS, BinaryKey(key.to_vec()))
            .map_err(LevelDB::to_storage_error)
    }

    fn put(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
        let result = self.db.put(LEVELDB_WRITE_OPTIONS, BinaryKey(key.to_vec()), &value);
        result.map_err(LevelDB::to_storage_error)
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), Error> {
        let result = self.db.delete(LEVELDB_WRITE_OPTIONS, BinaryKey(key.to_vec()));
        result.map_err(LevelDB::to_storage_error)
    }
    fn find_key(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let it = self.db.keys_iter(LEVELDB_READ_OPTIONS);
        it.seek(&BinaryKey(key.to_vec()));
        if it.valid() {
            let key = it.key();
            return Ok(Some(key.0));
        }
        Ok(None)
    }
}

impl Database for LevelDB {
    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        let mut batch = Writebatch::new();
        for (key, change) in patch.into_iter() {
            match change {
                Change::Put(ref v) => {
                    batch.put(BinaryKey(key.to_vec()), v);
                }
                Change::Delete => {
                    batch.delete(BinaryKey(key.to_vec()));
                }
            }
        }
        let write_opts = WriteOptions::new();
        let result = self.db.write(write_opts, &batch);
        result.map_err(LevelDB::to_storage_error)
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
