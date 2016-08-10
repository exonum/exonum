#[cfg(test)]
mod tests;

use num::{Integer, ToPrimitive};

use std::slice::SliceConcatExt;
use std::convert::AsRef;
use std::fmt::Debug;
// use std::iter::Iterator;

use ::crypto::Hash;
use ::messages::{Precommit, Propose, Message};

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
pub use self::db::{Database, Patch, Fork, Change};
pub use self::memorydb::MemoryDB;
pub use self::map_table::MapTable;
pub use self::list_table::ListTable;
pub use self::merkle_table::MerkleTable;
pub use self::fields::StorageValue;
pub use self::merkle_patricia_table::MerklePatriciaTable;

pub trait Storage<D: Database, T: Message + StorageValue> {
    fn db(&self) -> &D;
    fn db_mut(&mut self) -> &mut D;

    fn fork(&self) -> Fork<D> {
        self.db().fork()
    }

    fn transactions(&mut self) -> MapTable<D, Hash, T> {
        MapTable::new(vec![00], self.db_mut())
    }

    fn proposes(&mut self) -> MapTable<D, Hash, Propose> {
        MapTable::new(vec![01], self.db_mut())
    }

    fn heights(&mut self) -> ListTable<MapTable<D, [u8], Vec<u8>>, u64, Hash> {
        ListTable::new(MapTable::new(vec![02], self.db_mut()))
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

    fn precommits(&mut self, hash: &Hash)
        -> ListTable<MapTable<D, [u8], Vec<u8>>, u32, Precommit> {
        ListTable::new(MapTable::new([&[03], hash.as_ref()].concat(), self.db_mut()))
    }

    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        self.db_mut().merge(patch)
    }
}

pub trait Blockchain : Sized {
    type Database: Database;
    type Transaction: Message + StorageValue;

    fn db(&self) -> &Self::Database;
    fn db_mut(&mut self) -> &mut Self::Database;
}

impl<T, Tx, Db> Storage<Db, Tx> for T where T: Blockchain<Database=Db, Transaction=Tx>,
                                            Db: Database,
                                            Tx: Message + StorageValue {
    fn db(&self) -> &Db {
        Blockchain::db(self)
    }

    fn db_mut(&mut self) -> &mut Db {
        Blockchain::db_mut(self)
    }
}

impl<'a, Tx, Db> Storage<Fork<'a, Db>, Tx> for Fork<'a, Db> where Db: Database,
                                                                  Tx: Message + StorageValue {
    fn db(&self) -> &Fork<'a, Db> {
        self
    }

    fn db_mut(&mut self) -> &mut Fork<'a, Db> {
        self
    }
}

pub type Error = Box<Debug>;

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
