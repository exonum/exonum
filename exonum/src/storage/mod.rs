#[cfg(test)]
mod tests;

use num::{Integer, ToPrimitive};

use std::fmt;
use std::error;

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

#[derive(Debug)]
pub struct Error {
    message: String,
}

pub type Result<T> = ::std::result::Result<T, Error>;

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
    fn get(&self, key: &K) -> Result<Option<V>>;
    fn put(&self, key: &K, value: V) -> Result<()>;
    fn delete(&self, key: &K) -> Result<()>;
    fn find_key(&self, key: &K) -> Result<Option<Vec<u8>>>;
}

pub trait List<K: Integer + Copy + Clone + ToPrimitive, V> {
    fn append(&self, value: V) -> Result<()>;
    fn extend<I: IntoIterator<Item = V>>(&self, iter: I) -> Result<()>;
    fn get(&self, index: K) -> Result<Option<V>>;
    fn set(&self, index: K, value: V) -> Result<()>;
    fn last(&self) -> Result<Option<V>>;
    fn is_empty(&self) -> Result<bool>;
    fn len(&self) -> Result<K>;
}

impl Error {
    pub fn new<T: Into<String>>(message: T) -> Error {
        Error {
            message: message.into()
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Storage error: {}", self.message)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        &self.message
    }
}
