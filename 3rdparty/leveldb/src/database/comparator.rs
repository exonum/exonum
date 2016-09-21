//! All keys in leveldb are compared by their binary value unless
//! defined otherwise.
//!
//! Comparators allow to override this comparison.
//! The ordering of keys introduced by the compartor influences iteration order.
//! Databases written with one Comparator cannot be opened with another.
use leveldb_sys::*;
use libc::{size_t, c_char};
use libc;
use std::mem;
use std::slice;
use std::cmp::Ordering;

/// A comparator has two important functions:
///
/// * the name function returns a fixed name to detect errors when
///   opening databases with a different name
/// * The comparison implementation
pub trait Comparator {
    /// Return the name of the Comparator
    fn name(&self) -> *const c_char;
    /// compare two keys. This must implement a total ordering.
    fn compare(&self, a: &[u8], b: &[u8]) -> Ordering;
    /// whether the comparator is the `DefaultComparator`
    fn null() -> bool {
        false
    }
}

/// `OrdComparator` is a comparator comparing keys that implement `Ord`
pub struct OrdComparator {
    name: String,
}

impl OrdComparator {
    /// Create a new `OrdComparator`
    pub fn new(name: &str) -> OrdComparator {
        OrdComparator { name: name.to_string() }
    }
}
/// `DefaultComparator` is the a stand in for "no comparator set"
#[derive(Copy,Clone)]
pub struct DefaultComparator;

extern "C" fn name<T: Comparator>(state: *mut libc::c_void) -> *const c_char {
    let x: &T = unsafe { &*(state as *mut T) };
    x.name()
}

extern "C" fn compare<T: Comparator>(state: *mut libc::c_void,
                                     a: *const i8,
                                     a_len: size_t,
                                     b: *const i8,
                                     b_len: size_t)
                                     -> i32 {
    unsafe {
        let a_slice = slice::from_raw_parts::<u8>(a as *const u8, a_len as usize);
        let b_slice = slice::from_raw_parts::<u8>(b as *const u8, b_len as usize);
        let x: &T = &*(state as *mut T);
        let a_key = a_slice;
        let b_key = b_slice;
        match x.compare(a_key, b_key) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }
}

extern "C" fn destructor<T>(state: *mut libc::c_void) {
    let _x: Box<T> = unsafe { mem::transmute(state) };
    // let the Box fall out of scope and run the T's destructor
}

#[allow(missing_docs)]
pub fn create_comparator<T: Comparator>(x: Box<T>) -> *mut leveldb_comparator_t {
    unsafe {
        leveldb_comparator_create(mem::transmute(x), destructor::<T>, compare::<T>, name::<T>)
    }
}

impl Comparator for OrdComparator {
    fn name(&self) -> *const c_char {
        let slice: &str = self.name.as_ref();
        slice.as_ptr() as *const c_char
    }

    fn compare(&self, a: &[u8], b: &[u8]) -> Ordering {
        a.cmp(b)
    }
}
