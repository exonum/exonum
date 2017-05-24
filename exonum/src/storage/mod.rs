use serde_json;

use std::fmt;
use std::error::Error as ErrorTrait;
use std::convert;

pub use leveldb::options::Options as LevelDBOptions;
pub use leveldb::database::cache::Cache as LevelDBCache;

pub use self::leveldb::{LevelDB, LevelDBView};
pub use self::db::{Database, Patch, Fork, Change};
pub use self::memorydb::{MemoryDB, MemoryDBView};
pub use self::map_table::MapTable;
pub use self::list_table::ListTable;
pub use self::fields::StorageValue;
pub use self::merkle_table::MerkleTable;
pub use self::merkle_table::proofnode::Proofnode;
pub use self::merkle_patricia_table::MerklePatriciaTable;
pub use self::merkle_patricia_table::proofpathtokey::RootProofNode;
pub use self::utils::bytes_to_hex;

#[cfg(test)]
mod tests;
mod leveldb;
mod memorydb;
mod map_table;
mod list_table;
mod merkle_table;
mod fields;
mod db;
mod merkle_patricia_table;
mod utils;

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

pub trait List<V> {
    fn append(&self, value: V) -> Result<()>;
    fn extend<I: IntoIterator<Item = V>>(&self, iter: I) -> Result<()>;
    fn get(&self, index: u64) -> Result<Option<V>>;
    fn set(&self, index: u64, value: V) -> Result<()>;
    fn last(&self) -> Result<Option<V>>;
    fn is_empty(&self) -> Result<bool>;
    fn len(&self) -> Result<u64>;
    fn swap(&self, i: u64, j: u64) -> Result<()> {
        let first_val = self.get(i)?;
        let second_val = self.get(j)?;
        match (first_val, second_val) {
            (Some(i_val), Some(j_val)) => {
                self.set(j, i_val)?;
                self.set(i, j_val)?;
                Ok(())
            }
            _ => {
                Err(Error::new("One of swap indexes is not present in list"))
            }
        }
    }
}

impl Error {
    pub fn new<T: Into<String>>(message: T) -> Error {
        Error { message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Storage error: {}", self.message)
    }
}

impl ErrorTrait for Error {
    fn description(&self) -> &str {
        &self.message
    }
}

impl convert::From<serde_json::error::Error> for Error {
    fn from(message: serde_json::error::Error) -> Error {
        Error::new(message.description())
    }
}

#[cfg(not(feature="memorydb"))]
mod details {
    use super::{LevelDB, LevelDBView};

    pub type Storage = LevelDB;
    pub type View = LevelDBView;
}

#[cfg(feature="memorydb")]
mod details {
    use super::{MemoryDB, MemoryDBView};

    pub type Storage = MemoryDB;
    pub type View = MemoryDBView;
}

pub type Storage = details::Storage;
pub type View = details::View;
