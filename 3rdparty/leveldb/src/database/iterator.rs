//! leveldb iterators
//!
//! Iteration is one of the most important parts of leveldb. This module provides
//! Iterators to iterate over key, values and pairs of both.
use leveldb_sys::{leveldb_iterator_t, leveldb_iter_seek_to_first, leveldb_iter_destroy,
                  leveldb_iter_seek_to_last, leveldb_create_iterator, leveldb_iter_valid,
                  leveldb_iter_next, leveldb_iter_key, leveldb_iter_value,
                  leveldb_readoptions_destroy, leveldb_iter_seek};
use libc::{size_t, c_char};
use std::iter;
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
    start: bool,
    // Iterator accesses the Database through a leveldb_iter_t pointer
    // but needs to hold the reference for lifetime tracking
    #[allow(dead_code)]
    database: &'a Database,
    iter: RawIterator,
    from: Option<&'a [u8]>,
    to: Option<&'a [u8]>,
}

/// An iterator over the leveldb keyspace.
///
/// Returns just the keys.
pub struct KeyIterator<'a> {
    start: bool,
    // Iterator accesses the Database through a leveldb_iter_t pointer
    // but needs to hold the reference for lifetime tracking
    #[allow(dead_code)]
    database: &'a Database,
    iter: RawIterator,
    from: Option<&'a [u8]>,
    to: Option<&'a [u8]>,
}

/// An iterator over the leveldb keyspace.
///
/// Returns just the value.
pub struct ValueIterator<'a> {
    start: bool,
    // Iterator accesses the Database through a leveldb_iter_t pointer
    // but needs to hold the reference for lifetime tracking
    #[allow(dead_code)]
    database: &'a Database,
    iter: RawIterator,
    from: Option<&'a [u8]>,
    to: Option<&'a [u8]>,
}


/// A trait to allow access to the three main iteration styles of leveldb.
pub trait Iterable<'a> {
    /// Return an Iterator iterating over (Key,Value) pairs
    fn iter(&'a self, options: ReadOptions<'a>) -> Iterator;
    /// Returns an Iterator iterating over Keys only.
    fn keys_iter(&'a self, options: ReadOptions<'a>) -> KeyIterator;
    /// Returns an Iterator iterating over Values only.
    fn value_iter(&'a self, options: ReadOptions<'a>) -> ValueIterator;
}

impl<'a> Iterable<'a> for Database {
    fn iter(&'a self, options: ReadOptions<'a>) -> Iterator {
        Iterator::new(self, options)
    }

    fn keys_iter(&'a self, options: ReadOptions<'a>) -> KeyIterator {
        KeyIterator::new(self, options)
    }

    fn value_iter(&'a self, options: ReadOptions<'a>) -> ValueIterator {
        ValueIterator::new(self, options)
    }
}

#[allow(missing_docs)]
pub trait LevelDBIterator<'a> {
    #[inline]
    fn raw_iterator(&self) -> *mut leveldb_iterator_t;

    #[inline]
    fn start(&self) -> bool;

    #[inline]
    fn started(&mut self);

    fn from(self, key: &'a [u8]) -> Self;
    fn to(self, key: &'a [u8]) -> Self;

    fn from_key(&self) -> Option<&'a [u8]>;
    fn to_key(&self) -> Option<&'a [u8]>;

    fn valid(&self) -> bool {
        unsafe { leveldb_iter_valid(self.raw_iterator()) != 0 }
    }

    fn advance(&mut self) -> bool {
        unsafe {
            if !self.start() {

                leveldb_iter_next(self.raw_iterator());
            } else {
                if let Some(k) = self.from_key() {
                    self.seek(k)
                }
                self.started();
            }
        }
        self.valid()
    }

    fn key(&self) -> &'a [u8] {
        unsafe {
            let length: size_t = 0;
            let value = leveldb_iter_key(self.raw_iterator(), &length) as *const u8;
            from_raw_parts(value, length as usize)
        }
    }

    fn value(&self) -> Vec<u8> {
        unsafe {
            let length: size_t = 0;
            let value = leveldb_iter_value(self.raw_iterator(), &length) as *const u8;
            from_raw_parts(value, length as usize).to_vec()
        }
    }

    fn seek_to_first(&self) {
        unsafe { leveldb_iter_seek_to_first(self.raw_iterator()) }
    }

    fn seek_to_last(&self) {
        if let Some(k) = self.to_key() {
            self.seek(k);
        } else {
            unsafe {
                leveldb_iter_seek_to_last(self.raw_iterator());
            }
        }
    }

    fn seek<K: AsRef<[u8]>>(&self, key: K) {
        unsafe {
            let k = key.as_ref();
            leveldb_iter_seek(self.raw_iterator(),
                              k.as_ptr() as *mut c_char,
                              k.len() as size_t);
        }
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
                start: true,
                iter: RawIterator { ptr: ptr },
                database: database,
                from: None,
                to: None,
            }
        }
    }

    /// return the last element of the iterator
    pub fn last(self) -> Option<(&'a [u8], Vec<u8>)> {
        self.seek_to_last();
        Some((self.key(), self.value()))
    }
}

impl<'a> LevelDBIterator<'a> for Iterator<'a> {
    #[inline]
    fn raw_iterator(&self) -> *mut leveldb_iterator_t {
        self.iter.ptr
    }

    #[inline]
    fn start(&self) -> bool {
        self.start
    }

    #[inline]
    fn started(&mut self) {
        self.start = false
    }

    fn from(mut self, key: &'a [u8]) -> Self {
        self.from = Some(key);
        self
    }

    fn to(mut self, key: &'a [u8]) -> Self {
        self.to = Some(key);
        self
    }

    fn from_key(&self) -> Option<&'a [u8]> {
        self.from
    }

    fn to_key(&self) -> Option<&'a [u8]> {
        self.to
    }
}

impl<'a> KeyIterator<'a> {
    fn new(database: &'a Database, options: ReadOptions<'a>) -> KeyIterator<'a> {
        unsafe {
            let c_readoptions = c_readoptions(&options);
            let ptr = leveldb_create_iterator(database.database.ptr, c_readoptions);
            leveldb_readoptions_destroy(c_readoptions);
            leveldb_iter_seek_to_first(ptr);
            KeyIterator {
                start: true,
                iter: RawIterator { ptr: ptr },
                database: database,
                from: None,
                to: None,
            }
        }
    }

    /// return the last element of the iterator
    pub fn last(self) -> Option<&'a [u8]> {
        self.seek_to_last();
        Some(self.key())
    }
}

impl<'a> LevelDBIterator<'a> for KeyIterator<'a> {
    #[inline]
    fn raw_iterator(&self) -> *mut leveldb_iterator_t {
        self.iter.ptr
    }

    #[inline]
    fn start(&self) -> bool {
        self.start
    }

    #[inline]
    fn started(&mut self) {
        self.start = false
    }

    fn from(mut self, key: &'a [u8]) -> Self {
        self.from = Some(key);
        self
    }

    fn to(mut self, key: &'a [u8]) -> Self {
        self.to = Some(key);
        self
    }

    fn from_key(&self) -> Option<&'a [u8]> {
        self.from
    }

    fn to_key(&self) -> Option<&'a [u8]> {
        self.to
    }
}

impl<'a> ValueIterator<'a> {
    fn new(database: &'a Database, options: ReadOptions<'a>) -> ValueIterator<'a> {
        unsafe {
            let c_readoptions = c_readoptions(&options);
            let ptr = leveldb_create_iterator(database.database.ptr, c_readoptions);
            leveldb_readoptions_destroy(c_readoptions);
            leveldb_iter_seek_to_first(ptr);
            ValueIterator {
                start: true,
                iter: RawIterator { ptr: ptr },
                database: database,
                from: None,
                to: None,
            }
        }
    }

    /// return the last element of the iterator
    pub fn last(self) -> Option<Vec<u8>> {
        self.seek_to_last();
        Some(self.value())
    }
}

impl<'a> LevelDBIterator<'a> for ValueIterator<'a> {
    #[inline]
    fn raw_iterator(&self) -> *mut leveldb_iterator_t {
        self.iter.ptr
    }

    #[inline]
    fn start(&self) -> bool {
        self.start
    }

    #[inline]
    fn started(&mut self) {
        self.start = false
    }

    fn from(mut self, key: &'a [u8]) -> Self {
        self.from = Some(key);
        self
    }

    fn to(mut self, key: &'a [u8]) -> Self {
        self.to = Some(key);
        self
    }

    fn from_key(&self) -> Option<&'a [u8]> {
        self.from
    }

    fn to_key(&self) -> Option<&'a [u8]> {
        self.to
    }
}

impl<'a> iter::Iterator for Iterator<'a> {
    type Item = (&'a [u8], Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.advance() {
            Some((self.key(), self.value()))
        } else {
            None
        }
    }
}

impl<'a> iter::Iterator for KeyIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.advance() {
            Some(self.key())
        } else {
            None
        }
    }
}

impl<'a> iter::Iterator for ValueIterator<'a> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Vec<u8>> {
        if self.advance() {
            Some(self.value())
        } else {
            None
        }
    }
}
