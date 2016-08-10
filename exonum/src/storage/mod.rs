#[cfg(test)]
mod tests;

use num::{Integer, ToPrimitive};

use std::slice::SliceConcatExt;
use std::convert::AsRef;
use std::fmt::Debug;
// use std::iter::Iterator;

use ::crypto::Hash;
use ::messages::{TxMessage, Precommit, Propose};

mod leveldb;
mod memorydb;
mod map_table;
mod list_table;
mod merkle_table;
mod fields;
mod db;
mod merkle_patricia_table;

pub use leveldb::options::Options as LevelDBOptions;
pub use leveldb::database::cache::Cache as LevelDBCache;

pub use self::leveldb::LevelDB;
pub use self::db::{Database, Fork, Patch, Change};
pub use self::memorydb::MemoryDB;
pub use self::map_table::MapTable;
pub use self::list_table::ListTable;
pub use self::merkle_table::MerkleTable;
pub use self::fields::StorageValue;
pub use self::merkle_patricia_table::MerklePatriciaTable;

pub trait Blockchain {
    type Database: Database;
    type Transaction: StorageValue;

    // TODO: type Error;

    fn transactions<'a>(&'a mut self) -> MapTable<'a, Self::Database, Hash, Self::Transaction>;

    fn proposes<'a>(&'a mut self) -> MapTable<'a, Self::Database, Hash, Propose>;

    fn heights<'a>(&'a mut self) -> ListTable<MapTable<'a, Self::Database, [u8], Vec<u8>>, u64, Hash>;

    fn last_hash(&mut self) -> Result<Option<Hash>, Error>;

    fn last_propose(&mut self) -> Result<Option<Propose>, Error>;

    fn precommits<'a>(&'a mut self,
                          hash: &'a Hash)
                          -> ListTable<MapTable<'a, Self::Database, [u8], Vec<u8>>, u32, Precommit>;

    fn fork<'a>(&'a self) -> Storage<Fork<'a, Self::Database>>;

    fn merge(&mut self, patch: Patch) -> Result<(), Error>;
}

pub struct Storage<T: Database> {
    db: T,
}

pub type Error = Box<Debug>;

impl<T> Storage<T> where T: Database {
    pub fn new(backend: T) -> Self {
        Storage { db: backend }
    }
}

impl<D: Database> Blockchain for Storage<D> {
    type Transaction = TxMessage;
    type Database = D;

    fn transactions<'a>(&'a mut self) -> MapTable<'a, Self::Database, Hash, Self::Transaction> {
        self.db.map(vec![00])
    }

    fn proposes<'a>(&'a mut self) -> MapTable<'a, Self::Database, Hash, Propose> {
        self.db.map(vec![01])
    }

    fn heights<'a>(&'a mut self) -> ListTable<MapTable<'a, Self::Database, [u8], Vec<u8>>, u64, Hash> {
        self.db.list(vec![02])
    }

    fn last_hash(&mut self) -> Result<Option<Hash>, Error> {
        self.heights().last()
    }

    fn last_propose(&mut self) -> Result<Option<Propose>, Error> {
        Ok(match self.last_hash()? {
            Some(hash) => Some(self.proposes().get(&hash)?.unwrap()),
            None => None,
        })

    }

    fn precommits<'a>(&'a mut self,
                          hash: &'a Hash)
                          -> ListTable<MapTable<'a, Self::Database, [u8], Vec<u8>>, u32, Precommit> {
        self.db.list([&[03], hash.as_ref()].concat())
    }

    fn fork<'a>(&'a self) -> Storage<Fork<'a, Self::Database>> {
        Storage { db: self.db.fork() }
    }

    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
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

// TODO We need to understand how to finish them
// pub trait Iterable {
//     type Iter: Iterator;

//     fn iter(self) -> Self::Iter;
// }

// pub trait Seekable<'a> {
//     type Item;
//     type Key: ?Sized;

//     fn seek(&mut self, key: &Self::Key) -> Option<Self::Item>;
// }

pub trait Map<K: ?Sized, V> {
    fn get(&self, key: &K) -> Result<Option<V>, Error>;
    fn put(&mut self, key: &K, value: V) -> Result<(), Error>;
    fn delete(&mut self, key: &K) -> Result<(), Error>;
    fn find_key(&self, key: &K) -> Result<Option<Vec<u8>>, Error>;
}

pub trait List<K: Integer + Copy + Clone + ToPrimitive, V> {
    fn append(&mut self, value: V) -> Result<(), Error>;
    fn extend<I: IntoIterator<Item = V>>(&mut self, iter: I) -> Result<(), Error>;
    fn get(&self, index: K) -> Result<Option<V>, Error>;
    fn last(&self) -> Result<Option<V>, Error>;
    fn is_empty(&self) -> Result<bool, Error>;
    fn len(&self) -> Result<K, Error>;
}

pub trait MapExt: Map<[u8], Vec<u8>> + Sized {
    fn list<'a, K, V>(&'a mut self,
                      prefix: Vec<u8>)
                      -> ListTable<MapTable<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Integer + Copy + Clone + ToPrimitive + StorageValue,
              V: StorageValue;

    fn map<'a, K: ?Sized, V>(&'a mut self, prefix: Vec<u8>) -> MapTable<'a, Self, K, V>;
    fn merkle_list<'a, K, V>(&'a mut self,
                             prefix: Vec<u8>)
                             -> MerkleTable<MapTable<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Integer + Copy + Clone + ToPrimitive + StorageValue,
              V: StorageValue;
    fn merkle_map<'a, K: ?Sized, V>
        (&'a mut self,
         prefix: Vec<u8>)
         -> MerklePatriciaTable<MapTable<'a, Self, [u8], Vec<u8>>, K, V>
        where K: AsRef<[u8]>,
              V: StorageValue;
}

// TODO MapExt looks too complex. Find more simple way.
impl<T> MapExt for T
    where T: Map<[u8], Vec<u8>> + Sized
{
    fn list<'a, K, V>(&'a mut self,
                      prefix: Vec<u8>)
                      -> ListTable<MapTable<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Integer + Copy + Clone + ToPrimitive + StorageValue,
              V: StorageValue
    {
        ListTable::new(self.map(prefix))
    }

    fn map<'a, K: ?Sized, V>(&'a mut self, prefix: Vec<u8>) -> MapTable<'a, Self, K, V> {
        MapTable::new(prefix, self)
    }
    fn merkle_list<'a, K, V>(&'a mut self,
                             prefix: Vec<u8>)
                             -> MerkleTable<MapTable<'a, Self, [u8], Vec<u8>>, K, V>
        where K: Integer + Copy + Clone + ToPrimitive + StorageValue,
              V: StorageValue
    {
        MerkleTable::new(self.map(prefix))
    }
    fn merkle_map<'a, K: ?Sized, V>
        (&'a mut self,
         prefix: Vec<u8>)
         -> MerklePatriciaTable<MapTable<'a, Self, [u8], Vec<u8>>, K, V>
        where K: AsRef<[u8]>,
              V: StorageValue
    {
        let map_table = self.map(prefix);
        MerklePatriciaTable::new(map_table)
    }
}
