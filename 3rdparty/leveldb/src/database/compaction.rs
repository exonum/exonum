//! Compaction
use super::Database;
use leveldb_sys::leveldb_compact_range;
use libc::{c_char, size_t};

pub trait Compaction<'a> {
    fn compact<A: AsRef<[u8]>, B: AsRef<[u8]>>(&self, start: A, limit: B);
}

impl<'a> Compaction<'a> for Database {
    fn compact<A: AsRef<[u8]>, B: AsRef<[u8]>>(&self, start: A, limit: B) {
        unsafe {
            let s = start.as_ref();
            let l = limit.as_ref();
            leveldb_compact_range(self.database.ptr,
                                  s.as_ptr() as *mut c_char,
                                  s.len() as size_t,
                                  l.as_ptr() as *mut c_char,
                                  l.len() as size_t);
        }
    }
}
