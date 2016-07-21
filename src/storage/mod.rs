#[cfg(test)]
mod tests;

use std::slice::SliceConcatExt;
use std::convert::AsRef;
use std::fmt::Debug;
use std::num::{Zero, One};
use std::ops::{Add, Sub};

use ::crypto::Hash;
use ::messages::{TxMessage, Precommit, Propose};

mod leveldb;
mod memorydb;
mod map_table;
mod list_table;
mod fields;
mod db;

pub use self::db::{Database, Fork, Patch, Change};
pub use self::memorydb::MemoryDB;
pub use self::leveldb::LevelDB;
pub use self::map_table::MapTable;
pub use self::list_table::ListTable;
pub use self::fields::{StorageValue};

pub struct Storage<T: Database> {
    db: T,
}

pub type Error = Box<Debug>;

impl<T> Storage<T>
    where T: Database
{
    pub fn new(backend: T) -> Self {
        Storage { db: backend }
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
            None => None,
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

impl<'a, T> Storage<Fork<'a, T>>
    where T: Database
{
    pub fn patch(self) -> Patch {
        self.db.patch()
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
        where K: Zero + One + Add<Output = K> + Sub<Output = K> + PartialEq + Copy + StorageValue,
              ::std::ops::Range<K>: ::std::iter::Iterator<Item = K>,
              V: StorageValue;

    fn map<'a, K: ?Sized, V>(&'a mut self, prefix: Vec<u8>) -> MapTable<'a, Self, K, V>;
}

//TODO MapExt looks too complex. Find more simple way.
impl<T> MapExt for T
    where T: Map<[u8], Vec<u8>> + Sized
{
    fn list<'a, K, V>(&'a mut self,
                      prefix: Vec<u8>)
                      -> ListTable<MapTable<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Zero + One + Add<Output = K> + Sub<Output = K> + PartialEq + Copy + StorageValue,
              ::std::ops::Range<K>: ::std::iter::Iterator<Item = K>,
              V: StorageValue
    {
        ListTable::new(self.map(prefix))
    }

    fn map<'a, K: ?Sized, V>(&'a mut self, prefix: Vec<u8>) -> MapTable<'a, Self, K, V> {
        MapTable::new(prefix, self)
    }
}
