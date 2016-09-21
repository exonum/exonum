use leveldb::database::Database;
use leveldb::database::kv::{KV};
use leveldb::options::{Options,WriteOptions};
use std::path::Path;
use tempdir::TempDir;

pub fn open_database(path: &Path, create_if_missing: bool) -> Database {
  let mut opts = Options::new();
  opts.create_if_missing = create_if_missing;
  match Database::open(path, opts) {
    Ok(db) => { db },
    Err(e) => { panic!("failed to open database: {:?}", e) }
  }
}

pub fn tmpdir(name: &str) -> TempDir {
  TempDir::new(name)
           .unwrap()
}

pub fn db_put_simple<K: AsRef<[u8]>>(database: &Database, key: K, val: &[u8]) {
  let write_opts = WriteOptions::new();
  match database.put(write_opts, key.as_ref(), val) {
    Ok(_) => { () },
    Err(e) => { panic!("failed to write to database: {:?}", e) }
  }
}

