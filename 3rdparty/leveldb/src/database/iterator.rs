//! leveldb iterators
//!
//! Iteration is one of the most important parts of leveldb. This module provides
//! Iterators to iterate over key, values and pairs of both.
use leveldb_sys::{leveldb_iterator_t, leveldb_iter_seek_to_first, leveldb_iter_destroy,
                  leveldb_iter_seek_to_last, leveldb_create_iterator, leveldb_iter_valid,
                  leveldb_iter_next, leveldb_iter_key, leveldb_iter_value,
                  leveldb_readoptions_destroy, leveldb_iter_seek};
use libc::{size_t, c_char};
use super::Database;
use super::options::{ReadOptions, c_readoptions};
use std::slice::from_raw_parts;

#[allow(missing_docs)]
struct RawIterator {
    ptr: *mut leveldb_iterator_t,
}

#[allow(missing_docs)]
impl Drop for RawIterator {
    fn drop(&mut self) {
        unsafe { leveldb_iter_destroy(self.ptr) }
    }
}

/// An iterator over the leveldb keyspace.
///
/// Returns key and value as a tuple.
pub struct Iterator<'a> {
    move_next: bool,
    // Iterator accesses the Database through a leveldb_iter_t pointer
    // but needs to hold the reference for lifetime tracking
    #[allow(dead_code)]
    database: &'a Database,
    iter: RawIterator,
}

/// A trait to allow access to the three main iteration styles of leveldb.
pub trait Iterable<'a> {
    /// Return an Iterator iterating over (Key,Value) pairs
    fn iter(&'a self, options: ReadOptions<'a>) -> Iterator;
}

impl<'a> Iterable<'a> for Database {
    fn iter(&'a self, options: ReadOptions<'a>) -> Iterator {
        Iterator::new(self, options)
    }
}

impl<'a> Iterator<'a> {
    fn new(database: &'a Database, options: ReadOptions<'a>) -> Iterator<'a> {
        unsafe {
            let c_readoptions = c_readoptions(&options);
            let ptr = leveldb_create_iterator(database.database.ptr, c_readoptions);
            leveldb_readoptions_destroy(c_readoptions);
            leveldb_iter_seek_to_first(ptr);
            Iterator {
                move_next: false,
                iter: RawIterator { ptr: ptr },
                database: database,
            }
        }
    }

    fn valid(&self) -> bool {
        unsafe {
            leveldb_iter_valid(self.iter.ptr) != 0
        }
    }

    fn advance(&mut self) {
        unsafe {
            leveldb_iter_next(self.iter.ptr);
        }
    }

    unsafe fn key(&self) -> &[u8] {
        let length: size_t = 0;
        let value = leveldb_iter_key(self.iter.ptr, &length) as *const u8;
        from_raw_parts(value, length as usize)
    }

    unsafe fn value(&self) -> &[u8] {
        let length: size_t = 0;
        let value = leveldb_iter_value(self.iter.ptr, &length) as *const u8;
        from_raw_parts(value, length as usize)
    }

    pub fn seek_to_first(&mut self) {
        unsafe { leveldb_iter_seek_to_first(self.iter.ptr) }
    }

    pub fn seek_to_last(&mut self) {
        unsafe { leveldb_iter_seek_to_last(self.iter.ptr); }
    }

    pub fn seek(&mut self, key: &[u8]) {
        unsafe {
            leveldb_iter_seek(self.iter.ptr,
                              key.as_ptr() as *mut c_char,
                              key.len() as size_t);
        }
    }

    pub fn next(&mut self) -> Option<(&[u8], &[u8])> {
        if self.move_next {
            self.advance();
        }
        self.move_next = true;
        if self.valid() {
            unsafe {
                Some((self.key(), self.value()))
            }
        } else {
            None
        }
    }

    pub fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        if self.move_next {
            self.advance();
        }
        self.move_next = false;
        if self.valid() {
            unsafe {
                Some((self.key(), self.value()))
            }
        } else {
            None
        }
    }
}
