use std::slice::SliceConcatExt;
use std::convert::AsRef;
use std::marker::PhantomData;
use std::cell::Cell;
use std::num::{Zero, One};
use std::ops::{Add, Sub};
use std::sync::Arc;
use std::collections::BTreeMap;
use std::mem;
use std::path::Path;
use std::fmt::Debug;

use byteorder::{ByteOrder, BigEndian};
use db_key::Key;
use leveldb::database::Database as LevelDatabase;
use leveldb::error::Error as LevelError;
use leveldb::options::{Options, WriteOptions, ReadOptions};
use leveldb::database::kv::KV;
use leveldb::database::batch::Writebatch;
use leveldb::batch::Batch;

use ::crypto::Hash;
use ::messages::{MessageBuffer, Message, TxMessage, Precommit, Propose};

pub struct Storage<T: Database> {
    db: T,
}

pub type StorageError = Box<Debug>;

impl<T> Storage<T>
    where T: Database
{
    pub fn new(backend: T) -> Self {
        Storage {
            db: backend
        }
    }

    pub fn transactions<'a>(&'a mut self) -> MapTable<'a, T, Hash, TxMessage> {
        self.db.map(vec![00])
    }

    pub fn proposes<'a>(&'a mut self) -> MapTable<'a, T, Hash, Propose> {
        self.db.map(vec![01])
    }

    pub fn heights<'a>(&'a mut self) -> ListTable<MapTable<'a, T, [u8], Vec<u8>>, u64, Hash> {
        self.db.list(vec![02])
    }

    pub fn last_hash(&mut self) -> Result<Option<Hash>, StorageError> {
        self.heights().last()
    }

    pub fn last_propose(&mut self) -> Result<Option<Propose>, StorageError> {
        Ok(match self.last_hash()? {
            Some(hash) => Some(self.proposes().get(&hash)?.unwrap()),
            None => None
        })

    }

    pub fn precommits<'a>(&'a mut self,
                      hash: &'a Hash)
                      -> ListTable<MapTable<'a, T, [u8], Vec<u8>>, u32, Precommit> {
        self.db.list([&[03], hash.as_ref()].concat())
    }

    pub fn fork<'a>(&'a self) -> Storage<Fork<'a, T>> {
        Storage { db: self.db.fork() }
    }

    pub fn merge(&mut self, patch: Patch) -> Result<(), StorageError> {
        self.db.merge(patch)
    }
}

impl<'a, T> Storage<Fork<'a, T>> where T: Database {
    pub fn patch(self) -> Patch {
        self.db.patch()
    }
}

//TODO In this implementation there are extra memory allocations when key is passed into specific database.
//Think about key type. Maybe we can use keys with fixed length?
pub trait Database: Map<[u8], Vec<u8>> + Sized {
    fn fork<'a>(&'a self) -> Fork<'a, Self> {
        Fork {
            database: self,
            changes: BTreeMap::new(),
        }
    }

    fn merge(&mut self, patch: Patch) -> Result<(), StorageError>;
}

pub enum Change {
    Put(Vec<u8>),
    Delete,
}

pub struct Patch {
    changes: BTreeMap<Vec<u8>, Change>,
}

pub struct Fork<'a, T: Database + 'a> {
    database: &'a T,
    changes: BTreeMap<Vec<u8>, Change>,
}

impl<'a, T: Database + 'a> Fork<'a, T> {
    fn patch(self) -> Patch {
        Patch { changes: self.changes }
    }
}

impl<'a, T> Map<[u8], Vec<u8>> for Fork<'a, T>
    where T: Database + 'a {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        match self.changes.get(key) {
            Some(change) => {
                let v = match *change {
                    Change::Put(ref v) => Some(v.clone()),
                    Change::Delete => None,
                };
                Ok(v)
            }
            None => self.database.get(key),
        }
    }

    fn put(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), StorageError> {
        self.changes.insert(key.to_vec(), Change::Put(value));
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        self.changes.insert(key.to_vec(), Change::Delete);
        Ok(())
    }
}

impl<'a, T: Database + 'a + ?Sized> Database for Fork<'a, T> {
    fn merge(&mut self, patch: Patch) -> Result<(), StorageError> {
        self.changes.extend(patch.changes.into_iter());
        Ok(())
    }
}

pub struct MemoryDB {
    map: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl MemoryDB {
    pub fn new() -> MemoryDB {
        MemoryDB { map: BTreeMap::new() }
    }
}

impl Map<[u8], Vec<u8>> for MemoryDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.map.get(key).map(Clone::clone))
    }

    fn put(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), StorageError> {
        self.map.insert(key.to_vec(), value);
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        self.map.remove(key);
        Ok(())
    }
}

impl Database for MemoryDB {
    fn merge(&mut self, patch: Patch) -> Result<(), StorageError> {
        for (key, change) in patch.changes.into_iter() {
            match change {
                Change::Put(ref v) => {
                    self.map.insert(key.clone(), v.clone());
                }
                Change::Delete => {
                    self.map.remove(&key);
                }
            }
        }
        Ok(())
    }
}

struct BinaryKey(Vec<u8>);

impl Key for BinaryKey {
    fn from_u8(key: &[u8]) -> BinaryKey {
        BinaryKey(key.to_vec())
    }

    fn as_slice<T, F: Fn(&[u8]) -> T>(&self, f: F) -> T {
        f(&self.0)
    }
}

struct LevelDB {
    db: LevelDatabase<BinaryKey>,
}

const LEVELDB_READ_OPTIONS: ReadOptions<'static, BinaryKey> = ReadOptions {
    verify_checksums: false,
    fill_cache: true,
    snapshot: None,
};
const LEVELDB_WRITE_OPTIONS: WriteOptions = WriteOptions { sync: false };

impl LevelDB {
    fn new(path: &Path, options: Options) -> Result<LevelDB, StorageError> {
        match LevelDatabase::open(path, options) {
            Ok(database) => Ok(LevelDB { db: database }),
            Err(e) => Err(Self::to_storage_error(e)),
        }
    }

    fn to_storage_error(err: LevelError) -> StorageError {
        Box::new(err) as Box<Debug>
    }
}

impl Map<[u8], Vec<u8>> for LevelDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.db
            .get(LEVELDB_READ_OPTIONS, BinaryKey(key.to_vec()))
            .map_err(LevelDB::to_storage_error)
    }

    fn put(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), StorageError> {
        let result = self.db.put(LEVELDB_WRITE_OPTIONS, BinaryKey(key.to_vec()), &value);
        result.map_err(LevelDB::to_storage_error)
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        let result = self.db.delete(LEVELDB_WRITE_OPTIONS, BinaryKey(key.to_vec()));
        result.map_err(LevelDB::to_storage_error)
    }
}

impl Database for LevelDB {
    fn merge(&mut self, patch: Patch) -> Result<(), StorageError> {
        let mut batch = Writebatch::new();
        for (key, change) in patch.changes.into_iter() {
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

pub trait StorageValue {
    fn serialize(self) -> Vec<u8>;
    fn deserialize(v: Vec<u8>) -> Self;
}

impl StorageValue for u32 {
    // TODO: return Cow<[u8]>
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u32>()];
        BigEndian::write_u32(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u32(&v)
    }
}

impl StorageValue for u64 {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u64>()];
        BigEndian::write_u64(&mut v, self);
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        BigEndian::read_u64(&v)
    }
}

impl StorageValue for Hash {
    fn serialize(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Hash::from_slice(&v).unwrap()
    }
}

impl<T> StorageValue for T
    where T: Message
{
    fn serialize(self) -> Vec<u8> {
        self.raw().as_ref().as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        Message::from_raw(Arc::new(MessageBuffer::from_vec(v))).unwrap()
    }
}

impl StorageValue for TxMessage {
    fn serialize(self) -> Vec<u8> {
        self.raw().as_ref().as_ref().to_vec()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        TxMessage::from_raw(Arc::new(MessageBuffer::from_vec(v))).unwrap()
    }

}

impl StorageValue for Vec<u8> {
    fn serialize(self) -> Vec<u8> {
        self
    }

    fn deserialize(v: Vec<u8>) -> Self {
        v
    }
}

pub trait Map<K: ?Sized, V> {
    fn get(&self, key: &K) -> Result<Option<V>, StorageError>;
    fn put(&mut self, key: &K, value: V) -> Result<(), StorageError>;
    fn delete(&mut self, key: &K) -> Result<(), StorageError>;
}

pub struct MapTable<'a, T: Map<[u8], Vec<u8>> + 'a, K: ?Sized, V> {
    prefix: Vec<u8>,
    storage: &'a mut T,
    _k: PhantomData<K>,
    _v: PhantomData<V>,
}

impl<'a, T, K: ?Sized, V> Map<K, V> for MapTable<'a, T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: AsRef<[u8]>,
          V: StorageValue
{
    fn get(&self, key: &K) -> Result<Option<V>, StorageError> {
        let v = self.storage.get(&[&self.prefix, key.as_ref()].concat())?;
        Ok(v.map(StorageValue::deserialize))
    }

    fn put(&mut self, key: &K, value: V) -> Result<(), StorageError> {
        self.storage.put(&[&self.prefix, key.as_ref()].concat(), value.serialize())
    }

    fn delete(&mut self, key: &K) -> Result<(), StorageError> {
        self.storage.delete(&[&self.prefix, key.as_ref()].concat())
    }
}

trait MapExt: Map<[u8], Vec<u8>> + Sized {
    fn list<'a, K, V>(&'a mut self,
                      prefix: Vec<u8>)
                      -> ListTable<MapTable<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Zero + One + Add<Output = K> + Copy + StorageValue,
              V: StorageValue;

    fn map<'a, K: ?Sized, V>(&'a mut self, prefix: Vec<u8>) -> MapTable<'a, Self, K, V>;
}

impl<T> MapExt for T
    where T: Map<[u8], Vec<u8>> + Sized
{
    fn list<'a, K, V>(&'a mut self,
                      prefix: Vec<u8>)
                      -> ListTable<MapTable<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Copy + StorageValue,
              V: StorageValue
    {
        ListTable {
            map: self.map(prefix),
            count: Cell::new(None),
            _v: PhantomData,
        }
    }

    fn map<'a, K: ?Sized, V>(&'a mut self, prefix: Vec<u8>) -> MapTable<'a, Self, K, V> {
        MapTable {
            prefix: prefix,
            storage: self,
            _k: PhantomData,
            _v: PhantomData,
        }
    }
}

pub struct ListTable<T: Map<[u8], Vec<u8>>, K, V> {
    map: T,
    count: Cell<Option<K>>,
    _v: PhantomData<V>,
}

impl<'a, T, K, V> ListTable<T, K, V>
    where T: Map<[u8], Vec<u8>>,
          K: Zero + One + Add<Output = K> + Sub<Output = K> + PartialEq + Copy + StorageValue,
          ::std::ops::Range<K>: ::std::iter::Iterator<Item=K>,
          V: StorageValue
{
    pub fn append(&mut self, value: V) -> Result<(), StorageError> {
        let len = self.len()?;
        self.map.put(&len.serialize(), value.serialize());
        self.map.put(&[], (len + One::one()).serialize());
        self.count.set(Some(len + One::one()));
        Ok(())
    }

    pub fn extend<I>(&mut self, iter: I) -> Result<(), StorageError>
        where I: IntoIterator<Item = V>
    {
        let mut len = self.len()?;
        for value in iter {
            self.map.put(&len.serialize(), value.serialize());
            len = len + One::one();
        }
        self.map.put(&[], (len + One::one()).serialize());
        self.count.set(Some(len + One::one()));
        Ok(())
    }

    pub fn get(&self, index: K) -> Result<Option<V>, StorageError> {
        let value = self.map.get(&index.serialize())?;
        Ok(value.map(StorageValue::deserialize))
    }

    pub fn last(&self) -> Result<Option<V>, StorageError> {
        let len = self.len()?;
        if len == Zero::zero() {
            Ok(None)
        } else {
            self.get(len - One::one())
        }
    }

    // TODO: implement iterator for List
    pub fn iter(&self) -> Result<Option<Vec<V>>, StorageError> {
        Ok(if self.is_empty()? {
            None
        } else {
            Some((Zero::zero()..self.len()?).map(|i| self.get(i).unwrap().unwrap()).collect())
        })
    }

    pub fn is_empty(&self) -> Result<bool, StorageError> {
        Ok(self.len()? == Zero::zero())
    }

    pub fn len(&self) -> Result<K, StorageError> {
        if let Some(count) = self.count.get() {
            return Ok(count);
        }

        let v = self.map.get(&[])?;
        let c = v.map(K::deserialize).unwrap_or(Zero::zero());
        self.count.set(Some(c));
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::Map;
    use super::Database;
    use super::MemoryDB;
    use super::LevelDB;
    use super::MapExt;
    use super::StorageValue;
    use super::StorageError;

    use tempdir::TempDir;
    use leveldb::options::Options;

    fn leveldb_database() -> LevelDB {
        let mut options = Options::new();
        options.create_if_missing = true;
        LevelDB::new(TempDir::new("da").unwrap().path(), options).unwrap()
    }

    fn test_map_simple<T: Map<[u8], Vec<u8>>>(mut db: T) -> Result<(), StorageError> {
        db.put(b"aba", vec![1, 2, 3])?;
        assert_eq!(db.get(b"aba")?, Some(vec![1, 2, 3]));
        assert_eq!(db.get(b"caba")?, None);

        db.put(b"caba", vec![50, 14])?;
        db.delete(b"aba")?;
        assert_eq!(db.get(b"aba")?, None);
        db.put(b"caba", vec![1, 2, 3, 117, 3])?;
        assert_eq!(db.get(b"caba")?, Some(vec![1, 2, 3, 117, 3]));
        Ok(())
    }

    fn test_database_merge<T: Database>(mut db: T) -> Result<(), StorageError> {
        db.put(b"ab", vec![1, 2, 3])?;
        db.put(b"aba", vec![14, 22, 3])?;
        db.put(b"caba", vec![34, 2, 3])?;
        db.put(b"abacaba", vec![1, 65])?;

        let patch;
        {
            let mut fork = db.fork();
            fork.delete(b"ab")?;
            fork.put(b"abacaba", vec![18, 34])?;
            fork.put(b"caba", vec![10])?;
            fork.put(b"abac", vec![117, 32, 64])?;
            fork.put(b"abac", vec![14, 12])?;
            fork.delete(b"abacaba")?;

            assert_eq!(fork.get(b"ab")?, None);
            assert_eq!(fork.get(b"caba")?, Some(vec![10]));
            assert_eq!(fork.get(b"abac")?, Some(vec![14, 12]));
            assert_eq!(fork.get(b"aba")?, Some(vec![14, 22, 3]));
            assert_eq!(fork.get(b"abacaba")?, None);

            patch = fork.patch();
        }
        assert_eq!(db.get(b"ab")?, Some(vec![1, 2, 3]));
        assert_eq!(db.get(b"aba")?, Some(vec![14, 22, 3]));
        assert_eq!(db.get(b"caba")?, Some(vec![34, 2, 3]));
        assert_eq!(db.get(b"abacaba")?, Some(vec![1, 65]));

        db.merge(patch)?;
        assert_eq!(db.get(b"ab")?, None);
        assert_eq!(db.get(b"caba")?, Some(vec![10]));
        assert_eq!(db.get(b"abac")?, Some(vec![14, 12]));
        assert_eq!(db.get(b"aba")?, Some(vec![14, 22, 3]));
        assert_eq!(db.get(b"abacaba")?, None);
        Ok(())
    }

    fn test_table_list<T: Database>(prefix: Vec<u8>, db: &mut T) -> Result<(), StorageError> {
        let mut list = db.list(prefix);
        assert_eq!(list.len()?, 0 as u64);
        list.append(vec![10])?;
        assert_eq!(list.get(0)?, Some(vec![10]));

        list.append(vec![15])?;
        assert_eq!(list.len()?, 2);
        assert_eq!(list.last()?, Some(vec![15]));

        let bound: u64 = 500;
        for i in 0..bound {
            list.append(StorageValue::serialize(i as u64))?;
        }
        assert_eq!(list.last()?, Some(StorageValue::serialize(bound - 1)));
        assert_eq!(list.len()?, 2 + bound);
        Ok(())
    }

    fn test_table_map<T: Database>(prefix: Vec<u8>, db: &mut T) -> Result<(), StorageError> {
        let map = db.map(prefix);
        test_map_simple(map)
    }

    #[test]
    fn serializer() {
        let a: u32 = 10;
        let b: u64 = 15;
        let c: Vec<u8> = vec![10, 15, 24, 2, 1];

        let a_s = a.serialize();
        let b_s = b.serialize();
        let c_s = c.clone().serialize();
        let c_d: Vec<u8> = StorageValue::deserialize(c_s);
        assert_eq!(a, StorageValue::deserialize(a_s));
        assert_eq!(b, StorageValue::deserialize(b_s));
        assert_eq!(c, c_d);
    }

    #[test]
    fn memory_database_simple() {
        let db = MemoryDB::new();
        test_map_simple(db).unwrap();
    }

    #[test]
    fn leveldb_database_simple() {
        let db = leveldb_database();
        test_map_simple(db).unwrap();
    }

    #[test]
    fn memory_database_merge() {
        let db = MemoryDB::new();
        test_database_merge(db).unwrap();
    }

    #[test]
    fn leveldb_database_merge() {
        let mut db = leveldb_database();
        test_database_merge(db).unwrap();
    }

    #[test]
    fn memorydb_table_list() {
        let mut db = MemoryDB::new();
        test_table_list(vec![01], &mut db).unwrap();
        test_table_list(vec![02], &mut db).unwrap();
    }

    #[test]
    fn leveldb_table_list() {
        let mut db = leveldb_database();
        test_table_list(vec![01], &mut db).unwrap();
        test_table_list(vec![02], &mut db).unwrap();
    }

    #[test]
    fn memorydb_table_map() {
        let mut db = MemoryDB::new();
        test_table_map(vec![01], &mut db).unwrap();
        test_table_map(vec![02], &mut db).unwrap();
    }

    #[test]
    fn leveldb_table_map() {
        let mut db = leveldb_database();
        test_table_map(vec![01], &mut db).unwrap();
        test_table_map(vec![02], &mut db).unwrap();
    }
}
