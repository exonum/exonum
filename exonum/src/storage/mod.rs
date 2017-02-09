#[cfg(test)]
mod tests;

use std::fmt;
use std::error;

use ::crypto::{Hash, HASH_SIZE, hash};

mod db;
mod leveldb;
mod memorydb;

mod base_table;
mod map_table;
mod list_table;
mod merkle_table;
mod merkle_patricia_table;

mod keys;
mod values;

mod utils; 

pub use leveldb::options::Options as LevelDBOptions;
pub use leveldb::database::cache::Cache as LevelDBCache;

pub use self::leveldb::{LevelDB, LevelDBView};
pub use self::db::{Database, Patch, Fork, Change};
pub use self::memorydb::{MemoryDB, MemoryDBView};
pub use self::map_table::MapTable;
pub use self::list_table::ListTable;
pub use self::keys::{StorageKey, VoidKey};
pub use self::values::{StorageValue};
pub use self::merkle_table::MerkleTable; 
pub use self::merkle_patricia_table::{MerklePatriciaTable};
pub use self::utils::bytes_to_hex; 

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

impl error::Error for Error {
    fn description(&self) -> &str {
        &self.message
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

pub fn merkle_hash(hashes: &[Hash]) -> Hash {    
    match hashes.len() {
        0 => Hash::default(),
        1 => hashes[0],
        n => {
            let (left, right) = hashes.split_at(n.next_power_of_two() / 2);
            // TODO: allocate on stack
            let mut v = Vec::with_capacity(HASH_SIZE * 2);
            v.extend_from_slice(merkle_hash(left).as_ref());
            v.extend_from_slice(merkle_hash(right).as_ref());
            hash(&v)
        }
    }    
}
