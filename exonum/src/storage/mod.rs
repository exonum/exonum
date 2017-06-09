pub use self::error::Error;
pub use self::db::{Database, Snapshot, Fork, Patch, Change, Iter};

pub use self::leveldb::{LevelDB, LevelDBOptions};
pub use self::memorydb::MemoryDB;

pub use self::keys::StorageKey;
pub use self::values::StorageValue;

pub use self::entry::Entry;

pub use self::base_index::{BaseIndex, BaseIndexIter};
pub use self::map_index::MapIndex;
pub use self::list_index::ListIndex;
pub use self::key_set_index::KeySetIndex;
pub use self::value_set_index::ValueSetIndex;
pub use self::proof_list_index::{ProofListIndex, ListProof};
pub use self::proof_map_index::{ProofMapIndex, MapProof};

pub type Result<T> = ::std::result::Result<T, Error>;

mod error;

mod db;
mod leveldb;
mod memorydb;

mod keys;
mod values;

mod entry;

pub mod base_index;

pub mod map_index;
pub mod list_index;
pub mod key_set_index;
pub mod value_set_index;
pub mod proof_list_index;
pub mod proof_map_index;
