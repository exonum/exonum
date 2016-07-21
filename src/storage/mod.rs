#[cfg(test)] mod tests;

use std::slice::SliceConcatExt;
use std::convert::AsRef;
use std::sync::Arc;
use std::collections::BTreeMap;
use std::mem;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::cell::Cell;
use std::num::{Zero, One};
use std::ops::{Add};

use byteorder::{ByteOrder, BigEndian};

use ::crypto::Hash;
use ::messages::{MessageBuffer, Message, TxMessage, Precommit, Propose};

mod leveldb;
mod memorydb;
mod map_table;
mod list_table;

pub use self::memorydb::{MemoryDB};
pub use self::leveldb::{LevelDB};
pub use self::map_table::{MapTable};
pub use self::list_table::{ListTable};

pub struct Storage<T: Database> {
    db: T,
}

pub type Error = Box<Debug>;

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

    pub fn last_hash(&mut self) -> Result<Option<Hash>, Error> {
        self.heights().last()
    }

    pub fn last_propose(&mut self) -> Result<Option<Propose>, Error> {
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

    pub fn merge(&mut self, patch: Patch) -> Result<(), Error> {
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

    fn merge(&mut self, patch: Patch) -> Result<(), Error>;
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
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
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

    fn put(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
        self.changes.insert(key.to_vec(), Change::Put(value));
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), Error> {
        self.changes.insert(key.to_vec(), Change::Delete);
        Ok(())
    }
}

impl<'a, T: Database + 'a + ?Sized> Database for Fork<'a, T> {
    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        self.changes.extend(patch.changes.into_iter());
        Ok(())
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
    fn get(&self, key: &K) -> Result<Option<V>, Error>;
    fn put(&mut self, key: &K, value: V) -> Result<(), Error>;
    fn delete(&mut self, key: &K) -> Result<(), Error>;
}

pub trait MapExt: Map<[u8], Vec<u8>> + Sized {
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