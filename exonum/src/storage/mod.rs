#[cfg(test)]
mod tests;

use num::{Integer, ToPrimitive};

use std::slice::SliceConcatExt;
use std::fmt::Debug;
use std::borrow::{Borrow, BorrowMut};
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

pub trait TxStorage<D: Database, T: Message + StorageValue> where Self: Borrow<D>+BorrowMut<D> {
    fn transactions(&mut self) -> MapTable<D, Hash, T> {
        MapTable::new(vec![00], self.borrow_mut())
    }
}

pub trait BlockStorage<D: Database> where Self: Borrow<D>+BorrowMut<D> {
    fn proposes(&mut self) -> MapTable<D, Hash, Propose> {
        MapTable::new(vec![01], self.borrow_mut())
    }

    fn heights(&mut self) -> ListTable<MapTable<D, [u8], Vec<u8>>, u64, Hash> {
        ListTable::new(MapTable::new(vec![02], self.borrow_mut()))
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
        ListTable::new(MapTable::new([&[03], hash.as_ref()].concat(), self.borrow_mut()))
    }
}

pub trait Blockchain : Sized
    where Self: Borrow<<Self as Blockchain>::Database>,
          Self: BorrowMut<<Self as Blockchain>::Database> {
    type Database: Database;
    type Transaction: Message + StorageValue;

    fn fork(&self) -> Fork<Self::Database> {
        self.borrow().fork()
    }

    fn merge(&mut self, patch: Patch) -> Result<(), Error> {
        self.borrow_mut().merge(patch)
    }
}

impl<T, Tx, Db> TxStorage<Db, Tx> for T
    where T: Blockchain<Database=Db, Transaction=Tx>,
          Db: Database,
          Tx: Message + StorageValue {}

impl<'a, Tx, Db> TxStorage<Fork<'a, Db>, Tx> for Fork<'a, Db>
    where Db: Database,
          Tx: Message + StorageValue {}

impl<T, Db, Tx> BlockStorage<Db> for T
    where T: Blockchain<Database=Db, Transaction=Tx>,
          Db: Database,
          Tx: Message + StorageValue {}

impl<'a, Db> BlockStorage<Fork<'a, Db>> for Fork<'a, Db>
    where Db: Database {}

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
