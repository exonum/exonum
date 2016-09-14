#[cfg(test)]
mod tests;

use num::{Integer, ToPrimitive};

use std::fmt::Debug;

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
    fn put(&self, key: &K, value: V) -> Result<(), Error>;
    fn delete(&self, key: &K) -> Result<(), Error>;
    fn find_key(&self, key: &K) -> Result<Option<Vec<u8>>, Error>;
}

pub trait List<K: Integer + Copy + Clone + ToPrimitive, V> {
    fn append(&self, value: V) -> Result<(), Error>;
    fn extend<I: IntoIterator<Item = V>>(&self, iter: I) -> Result<(), Error>;
    fn get(&self, index: K) -> Result<Option<V>, Error>;
    fn last(&self) -> Result<Option<V>, Error>;
    fn is_empty(&self) -> Result<bool, Error>;
    fn len(&self) -> Result<K, Error>;
}
