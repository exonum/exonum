//! Structs and traits to work with the leveldb cache.
use leveldb_sys::{leveldb_cache_t, leveldb_cache_create_lru, leveldb_cache_destroy};
use libc::size_t;

#[allow(missing_docs)]
struct RawCache {
    ptr: *mut leveldb_cache_t,
}

impl Drop for RawCache {
    fn drop(&mut self) {
        unsafe {
            leveldb_cache_destroy(self.ptr);
        }
    }
}

/// Represents a leveldb cache
pub struct Cache {
    raw: RawCache,
}

impl Cache {
    /// Create a leveldb LRU cache of a given size
    pub fn new(size: size_t) -> Cache {
        let cache = unsafe { leveldb_cache_create_lru(size) };
        Cache { raw: RawCache { ptr: cache } }
    }

    #[allow(missing_docs)]
    pub fn raw_ptr(&self) -> *mut leveldb_cache_t {
        self.raw.ptr
    }
}
