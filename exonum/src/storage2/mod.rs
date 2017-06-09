use ::crypto::{Hash, HASH_SIZE, hash};


pub use self::error::Error;
pub use self::db::{Database, Snapshot, Fork, Patch, Change, Iter};

pub use self::leveldb::LevelDB;
pub use self::memorydb::MemoryDB;

pub use self::keys::StorageKey;
pub use self::values::StorageValue;

pub use self::base_index::{BaseIndex, BaseIndexIter};
pub use self::entry::Entry;
pub use self::map_index::MapIndex;
pub use self::list_index::ListIndex;
pub use self::key_set_index::KeySetIndex;
pub use self::value_set_index::ValueSetIndex;
pub use self::proof_list_index::ProofListIndex;
pub use self::proof_map_index::ProofMapIndex;

pub type Result<T> = ::std::result::Result<T, Error>;

mod error;

mod db;
mod leveldb;
mod memorydb;

mod keys;
mod values;

mod base_index;
mod entry;
mod map_index;
mod list_index;
mod key_set_index;
mod value_set_index;
mod proof_list_index;
mod proof_map_index;

pub fn pair_hash(h1: &Hash, h2: &Hash) -> Hash {
    let mut v = [0; HASH_SIZE * 2];
    v[..HASH_SIZE].copy_from_slice(h1.as_ref());
    v[HASH_SIZE..].copy_from_slice(h2.as_ref());
    hash(&v)
}

pub fn merkle_hash(hashes: &[Hash]) -> Hash {
    match hashes.len() {
        0 => Hash::default(),
        1 => hashes[0],
        n => {
            let (l, r) = hashes.split_at(n.next_power_of_two() / 2);
            pair_hash(&merkle_hash(l), &merkle_hash(r))
        }
    }
}
