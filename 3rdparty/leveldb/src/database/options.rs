//! All the option types needed for interfacing with leveldb.
//!
//! Those are:
//! * `Options`: used when opening a database
//! * `ReadOptions`: used when reading from leveldb
//! * `WriteOptions`: used when writng to leveldb
use leveldb_sys::*;

use libc::size_t;
use database::snapshots::Snapshot;
use database::cache::Cache;

/// Options to consider when opening a new or pre-existing database.
///
/// Note that in contrast to the leveldb C API, the Comparator is not
/// passed using this structure.
///
/// For more detailed explanations, consider the
/// [leveldb documentation](https://github.com/google/leveldb/tree/master/doc)
pub struct Options {
    /// create the database if missing
    ///
    /// default: false
    pub create_if_missing: bool,
    /// report an error if the DB already exists instead of opening.
    ///
    /// default: false
    pub error_if_exists: bool,
    /// paranoid checks make the database report an error as soon as
    /// corruption is detected.
    ///
    /// default: false
    pub paranoid_checks: bool,
    /// Override the size of the write buffer to use.
    ///
    /// default: None
    pub write_buffer_size: Option<size_t>,
    /// Override the max number of open files.
    ///
    /// default: None
    pub max_open_files: Option<i32>,
    /// Override the size of the blocks leveldb uses for writing and caching.
    ///
    /// default: None
    pub block_size: Option<size_t>,
    /// Override the interval between restart points.
    ///
    /// default: None
    pub block_restart_interval: Option<i32>,
    /// Define whether leveldb should write compressed or not.
    ///
    /// default: Compression::No
    pub compression: Compression,
    /// A cache to use during read operations.
    ///
    /// default: None
    pub cache: Option<Cache>,
}

impl Options {
    /// Create a new `Options` struct with default settings.
    pub fn new() -> Options {
        Options {
            create_if_missing: false,
            error_if_exists: false,
            paranoid_checks: false,
            write_buffer_size: None,
            max_open_files: None,
            block_size: None,
            block_restart_interval: None,
            compression: Compression::No,
            cache: None,
        }
    }
}

impl Default for Options {
    fn default() -> Options {
        Options::new()
    }
}

/// The write options to use for a write operation.
#[derive(Copy, Clone)]
pub struct WriteOptions {
    /// `fsync` before acknowledging a write operation.
    ///
    /// default: false
    pub sync: bool,
}

impl WriteOptions {
    /// Return a new `WriteOptions` struct with default settings.
    pub fn new() -> WriteOptions {
        WriteOptions { sync: false }
    }
}

impl Default for WriteOptions {
    fn default() -> WriteOptions {
        WriteOptions::new()
    }
}

/// The read options to use for any read operation.
#[allow(missing_copy_implementations)]
pub struct ReadOptions<'a> {
    /// Whether to verify the saved checksums on read.
    ///
    /// default: false
    pub verify_checksums: bool,
    /// Whether to fill the internal cache with the
    /// results of the read.
    ///
    /// default: true
    pub fill_cache: bool,
    /// An optional snapshot to base this operation on.
    ///
    /// Consider using the `Snapshot` trait instead of setting
    /// this yourself.
    ///
    /// default: None
    pub snapshot: Option<&'a Snapshot<'a>>,
}

impl<'a> ReadOptions<'a> {
    /// Return a `ReadOptions` struct with the default values.
    pub fn new() -> ReadOptions<'a> {
        ReadOptions {
            verify_checksums: false,
            fill_cache: true,
            snapshot: None,
        }
    }
}

#[allow(missing_docs)]
pub unsafe fn c_options(options: &Options,
                        comparator: Option<*mut leveldb_comparator_t>)
                        -> *mut leveldb_options_t {
    let c_options = leveldb_options_create();
    leveldb_options_set_create_if_missing(c_options, options.create_if_missing as u8);
    leveldb_options_set_error_if_exists(c_options, options.error_if_exists as u8);
    leveldb_options_set_paranoid_checks(c_options, options.paranoid_checks as u8);
    if let Some(wbs) = options.write_buffer_size {
        leveldb_options_set_write_buffer_size(c_options, wbs);
    }
    if let Some(mf) = options.max_open_files {
        leveldb_options_set_max_open_files(c_options, mf);
    }
    if let Some(bs) = options.block_size {
        leveldb_options_set_block_size(c_options, bs);
    }
    if let Some(bi) = options.block_restart_interval {
        leveldb_options_set_block_restart_interval(c_options, bi);
    }
    leveldb_options_set_compression(c_options, options.compression);
    if let Some(c) = comparator {
        leveldb_options_set_comparator(c_options, c);
    }
    if let Some(ref cache) = options.cache {
        leveldb_options_set_cache(c_options, cache.raw_ptr());
    }
    c_options
}

#[allow(missing_docs)]
pub unsafe fn c_writeoptions(options: WriteOptions) -> *mut leveldb_writeoptions_t {
    let c_writeoptions = leveldb_writeoptions_create();
    leveldb_writeoptions_set_sync(c_writeoptions, options.sync as u8);
    c_writeoptions
}

#[allow(missing_docs)]
pub unsafe fn c_readoptions(options: &ReadOptions) -> *mut leveldb_readoptions_t {
    let c_readoptions = leveldb_readoptions_create();
    leveldb_readoptions_set_verify_checksums(c_readoptions, options.verify_checksums as u8);
    leveldb_readoptions_set_fill_cache(c_readoptions, options.fill_cache as u8);

    if let Some(snapshot) = options.snapshot {
        leveldb_readoptions_set_snapshot(c_readoptions, snapshot.raw_ptr());
    }
    c_readoptions
}
