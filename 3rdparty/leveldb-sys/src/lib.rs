#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]
#![allow(dead_code)]

extern crate libc;

use libc::{c_char, c_int, c_uchar, c_void};
use libc::size_t;

// These are opaque types that LevelDB uses.
pub enum leveldb_t {}
pub enum leveldb_cache_t {}
pub enum leveldb_comparator_t {}
pub enum leveldb_env_t {}
pub enum leveldb_filelock_t {}
pub enum leveldb_filterpolicy_t {}
pub enum leveldb_iterator_t {}
pub enum leveldb_logger_t {}
pub enum leveldb_options_t {}
pub enum leveldb_randomfile_t {}
pub enum leveldb_readoptions_t {}
pub enum leveldb_seqfile_t {}
pub enum leveldb_snapshot_t {}
pub enum leveldb_writablefile_t {}
pub enum leveldb_writebatch_t {}
pub enum leveldb_writeoptions_t {}

#[repr(C)]
#[derive(Copy,Clone)]
pub enum Compression {
  No = 0,
  Snappy = 1
}

extern "C" {
    // DB operations
    pub fn leveldb_open(options: *const leveldb_options_t, name: *const c_char, errptr: *mut *mut c_char) -> *mut leveldb_t;
    pub fn leveldb_close(db: *mut leveldb_t);
    pub fn leveldb_put(db: *mut leveldb_t, options: *const leveldb_writeoptions_t, key: *const c_char, keylen: size_t, val: *const c_char, vallen: size_t, errptr: *mut *mut c_char);
    pub fn leveldb_delete(db: *mut leveldb_t, options: *const leveldb_writeoptions_t, key: *const c_char, keylen: size_t, errptr: *mut *mut c_char);
    pub fn leveldb_write(db: *mut leveldb_t, options: *const leveldb_writeoptions_t, batch: *mut leveldb_writebatch_t, errptr: *mut *mut c_char);
    pub fn leveldb_get(db: *mut leveldb_t, options: *const leveldb_readoptions_t, key: *const c_char, keylen: size_t, vallen: *mut size_t, errptr: *mut *mut c_char) -> *mut c_char;
    pub fn leveldb_create_iterator(db: *mut leveldb_t, options: *const leveldb_readoptions_t) -> *mut leveldb_iterator_t;
    pub fn leveldb_create_snapshot(db: *mut leveldb_t) -> *mut leveldb_snapshot_t;
    pub fn leveldb_release_snapshot(db: *mut leveldb_t, snapshot: *const leveldb_snapshot_t);
    pub fn leveldb_property_value(db: *mut leveldb_t, propname: *const c_char) -> *mut c_char;

    // TODO: const'ness of pointers here is in question
    pub fn leveldb_approximate_sizes(db: *mut leveldb_t, num_ranges: c_int, range_start_key: *const *const c_char, range_start_key_len: *const size_t, range_limit_key: *const *const c_char, range_limit_key_len: *const size_t, sizes: *mut u64);
    pub fn leveldb_compact_range(db: *mut leveldb_t, start_key: *const c_char, start_key_len: size_t, limit_key: *const c_char, limit_key_len: size_t);

    // Management operations
    pub fn leveldb_destroy_db(options: *const leveldb_options_t, name: *const c_char, errptr: *mut *mut c_char);
    pub fn leveldb_repair_db(options: *const leveldb_options_t, name: *const c_char, errptr: *mut *mut c_char);

    // Iterator
    pub fn leveldb_iter_destroy(it: *mut leveldb_iterator_t);
    pub fn leveldb_iter_valid(it: *const leveldb_iterator_t) -> c_uchar;
    pub fn leveldb_iter_seek_to_first(it: *mut leveldb_iterator_t);
    pub fn leveldb_iter_seek_to_last(it: *mut leveldb_iterator_t);
    pub fn leveldb_iter_seek(it: *mut leveldb_iterator_t, k: *const c_char, klen: size_t);
    pub fn leveldb_iter_next(it: *mut leveldb_iterator_t);
    pub fn leveldb_iter_prev(it: *mut leveldb_iterator_t);
    pub fn leveldb_iter_key(it: *const leveldb_iterator_t, klen: *const size_t) -> *const c_char;
    pub fn leveldb_iter_value(it: *const leveldb_iterator_t, vlen: *const size_t) -> *const c_char;
    pub fn leveldb_iter_get_error(it: *const leveldb_iterator_t, errptr: *const *const c_char);

    // Write batch
    pub fn leveldb_writebatch_create() -> *mut leveldb_writebatch_t;
    pub fn leveldb_writebatch_destroy(b: *mut leveldb_writebatch_t);
    pub fn leveldb_writebatch_clear(b: *mut leveldb_writebatch_t);
    pub fn leveldb_writebatch_put(b: *mut leveldb_writebatch_t, key: *const c_char, keylen: size_t, val: *const c_char, vallen: size_t);
    pub fn leveldb_writebatch_delete(b: *mut leveldb_writebatch_t, key: *const c_char, keylen: size_t);
    pub fn leveldb_writebatch_iterate(
        b: *mut leveldb_writebatch_t,
        state: *mut c_void,
        put: extern fn(*mut c_void, *const c_char, size_t, *const c_char, size_t),
        deleted: extern fn(*mut c_void, *const c_char, size_t)
    );

    // Options
    pub fn leveldb_options_create() -> *mut leveldb_options_t;
    pub fn leveldb_options_destroy(o: *mut leveldb_options_t);
    pub fn leveldb_options_set_comparator(o: *mut leveldb_options_t, c: *mut leveldb_comparator_t);
    pub fn leveldb_options_set_filter_policy(o: *mut leveldb_options_t, c: *mut leveldb_filterpolicy_t);
    pub fn leveldb_options_set_create_if_missing(o: *mut leveldb_options_t, val: c_uchar);
    pub fn leveldb_options_set_error_if_exists(o: *mut leveldb_options_t, val: c_uchar);
    pub fn leveldb_options_set_paranoid_checks(o: *mut leveldb_options_t, val: c_uchar);
    pub fn leveldb_options_set_env(o: *mut leveldb_options_t, env: *mut leveldb_env_t);
    pub fn leveldb_options_set_info_log(o: *mut leveldb_options_t, logger: *mut leveldb_logger_t);
    pub fn leveldb_options_set_write_buffer_size(o: *mut leveldb_options_t, size: size_t);
    pub fn leveldb_options_set_max_open_files(o: *mut leveldb_options_t, num: c_int);
    pub fn leveldb_options_set_cache(o: *mut leveldb_options_t, cache: *mut leveldb_cache_t);
    pub fn leveldb_options_set_block_size(o: *mut leveldb_options_t, size: size_t);
    pub fn leveldb_options_set_block_restart_interval(o: *mut leveldb_options_t, interval: c_int);
    pub fn leveldb_options_set_compression(o: *mut leveldb_options_t, val: Compression);

    // Comparator
    pub fn leveldb_comparator_create(
        state: *mut c_void,
        destructor: extern fn(*mut c_void),
        compare: extern fn(*mut c_void, *const c_char, size_t, *const c_char, size_t) -> c_int,
        name: extern fn(*mut c_void) -> *const c_char
    ) -> *mut leveldb_comparator_t;
    pub fn leveldb_comparator_destroy(c: *mut leveldb_comparator_t);

    // Filter policy
    //pub leveldb_filterpolicy_create(state: *mut c_void, /* TODO */) -> *mut leveldb_filterpolicy_t;
    /*
        extern leveldb_filterpolicy_t* leveldb_filterpolicy_create(
            void* state,
            void (*destructor)(void*),
            char* (*create_filter)(
                void*,
                const char* const* key_array, const size_t* key_length_array,
                int num_keys,
                size_t* filter_length),
            unsigned char (*key_may_match)(
                void*,
                const char* key, size_t length,
                const char* filter, size_t filter_length),
            const char* (*name)(void*));
    */
    pub fn leveldb_filterpolicy_destroy(p: *mut leveldb_filterpolicy_t);
    pub fn leveldb_filterpolicy_create_bloom(bits_per_key: c_int) -> *mut leveldb_filterpolicy_t;

    // Read options
    pub fn leveldb_readoptions_create() -> *mut leveldb_readoptions_t;
    pub fn leveldb_readoptions_destroy(o: *mut leveldb_readoptions_t);
    pub fn leveldb_readoptions_set_verify_checksums(o: *mut leveldb_readoptions_t, val: c_uchar);
    pub fn leveldb_readoptions_set_fill_cache(o: *mut leveldb_readoptions_t, val: c_uchar);
    pub fn leveldb_readoptions_set_snapshot(o: *mut leveldb_readoptions_t, snapshot: *const leveldb_snapshot_t);

    // Write options
    pub fn leveldb_writeoptions_create() -> *mut leveldb_writeoptions_t;
    pub fn leveldb_writeoptions_destroy(o: *mut leveldb_writeoptions_t);
    pub fn leveldb_writeoptions_set_sync(o: *mut leveldb_writeoptions_t, val: c_uchar);

    // Cache
    pub fn leveldb_cache_create_lru(capacity: size_t) -> *mut leveldb_cache_t;
    pub fn leveldb_cache_destroy(c: *mut leveldb_cache_t);

    // Env
    pub fn leveldb_create_default_env() -> *mut leveldb_env_t;
    pub fn leveldb_env_destroy(e: *mut leveldb_env_t);

    // Utility
    pub fn leveldb_free(ptr: *mut c_void);

    // Versioning
    pub fn leveldb_major_version() -> c_int;
    pub fn leveldb_minor_version() -> c_int;
}
