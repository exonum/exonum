use ::crypto::{Hash, HASH_SIZE, hash};


pub use self::error::Error;
pub use self::db::{Database, Snapshot, Fork, Patch, Change};
pub use self::leveldb::LevelDB;


pub type Result<T> = ::std::result::Result<T, Error>;



mod error;
mod db;
mod leveldb;
mod memorydb;



pub trait Map<K: ?Sized, V> {
    fn get(&self, key: &K) -> Result<Option<V>>;
    fn put(&self, key: &K, value: V) -> Result<()>;
    fn delete(&self, key: &K) -> Result<()>;
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
