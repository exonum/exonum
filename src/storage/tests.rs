use super::{Map, List};
use super::Database;
use super::MemoryDB;
use super::LevelDB;
use super::MapExt;
use super::StorageValue;
use super::Error;

use tempdir::TempDir;
use leveldb::options::Options;

fn leveldb_database() -> LevelDB {
    let mut options = Options::new();
    options.create_if_missing = true;
    LevelDB::new(TempDir::new("da").unwrap().path(), options).unwrap()
}

fn test_map_simple<T: Map<[u8], Vec<u8>>>(mut db: T) -> Result<(), Error> {
    db.put(b"aba", vec![1, 2, 3])?;
    assert_eq!(db.get(b"aba")?, Some(vec![1, 2, 3]));
    assert_eq!(db.get(b"caba")?, None);

    db.put(b"caba", vec![50, 14])?;
    db.delete(b"aba")?;
    assert_eq!(db.get(b"aba")?, None);
    db.put(b"caba", vec![1, 2, 3, 117, 3])?;
    assert_eq!(db.get(b"caba")?, Some(vec![1, 2, 3, 117, 3]));
    Ok(())
}

fn test_database_merge<T: Database>(mut db: T) -> Result<(), Error> {
    db.put(b"ab", vec![1, 2, 3])?;
    db.put(b"aba", vec![14, 22, 3])?;
    db.put(b"caba", vec![34, 2, 3])?;
    db.put(b"abacaba", vec![1, 65])?;

    let patch;
    {
        let mut fork = db.fork();
        fork.delete(b"ab")?;
        fork.put(b"abacaba", vec![18, 34])?;
        fork.put(b"caba", vec![10])?;
        fork.put(b"abac", vec![117, 32, 64])?;
        fork.put(b"abac", vec![14, 12])?;
        fork.delete(b"abacaba")?;

        assert_eq!(fork.get(b"ab")?, None);
        assert_eq!(fork.get(b"caba")?, Some(vec![10]));
        assert_eq!(fork.get(b"abac")?, Some(vec![14, 12]));
        assert_eq!(fork.get(b"aba")?, Some(vec![14, 22, 3]));
        assert_eq!(fork.get(b"abacaba")?, None);

        patch = fork.patch();
    }
    assert_eq!(db.get(b"ab")?, Some(vec![1, 2, 3]));
    assert_eq!(db.get(b"aba")?, Some(vec![14, 22, 3]));
    assert_eq!(db.get(b"caba")?, Some(vec![34, 2, 3]));
    assert_eq!(db.get(b"abacaba")?, Some(vec![1, 65]));

    db.merge(patch)?;
    assert_eq!(db.get(b"ab")?, None);
    assert_eq!(db.get(b"caba")?, Some(vec![10]));
    assert_eq!(db.get(b"abac")?, Some(vec![14, 12]));
    assert_eq!(db.get(b"aba")?, Some(vec![14, 22, 3]));
    assert_eq!(db.get(b"abacaba")?, None);
    Ok(())
}

fn test_table_list<T: Database>(prefix: Vec<u8>, db: &mut T) -> Result<(), Error> {
    let mut list = db.list(prefix);
    assert_eq!(list.len()?, 0 as u64);
    list.append(vec![10])?;
    assert_eq!(list.get(0)?, Some(vec![10]));

    list.append(vec![15])?;
    assert_eq!(list.len()?, 2);
    assert_eq!(list.last()?, Some(vec![15]));

    let bound: u64 = 500;
    for i in 0..bound {
        list.append(StorageValue::serialize(i as u64))?;
    }
    assert_eq!(list.last()?, Some(StorageValue::serialize(bound - 1)));
    assert_eq!(list.len()?, 2 + bound);
    Ok(())
}

fn test_table_map<T: Database>(prefix: Vec<u8>, db: &mut T) -> Result<(), Error> {
    let map = db.map(prefix);
    test_map_simple(map)
}

#[test]
fn serializer() {
    let a: u32 = 10;
    let b: u64 = 15;
    let c: Vec<u8> = vec![10, 15, 24, 2, 1];

    let a_s = a.serialize();
    let b_s = b.serialize();
    let c_s = c.clone().serialize();
    let c_d: Vec<u8> = StorageValue::deserialize(c_s);
    assert_eq!(a, StorageValue::deserialize(a_s));
    assert_eq!(b, StorageValue::deserialize(b_s));
    assert_eq!(c, c_d);
}

#[test]
fn memory_database_simple() {
    let db = MemoryDB::new();
    test_map_simple(db).unwrap();
}

#[test]
fn leveldb_database_simple() {
    let db = leveldb_database();
    test_map_simple(db).unwrap();
}

#[test]
fn memory_database_merge() {
    let db = MemoryDB::new();
    test_database_merge(db).unwrap();
}

#[test]
fn leveldb_database_merge() {
    let db = leveldb_database();
    test_database_merge(db).unwrap();
}

#[test]
fn memorydb_table_list() {
    let mut db = MemoryDB::new();
    test_table_list(vec![01], &mut db).unwrap();
    test_table_list(vec![02], &mut db).unwrap();
}

#[test]
fn leveldb_table_list() {
    let mut db = leveldb_database();
    test_table_list(vec![01], &mut db).unwrap();
    test_table_list(vec![02], &mut db).unwrap();
}

#[test]
fn memorydb_table_map() {
    let mut db = MemoryDB::new();
    test_table_map(vec![01], &mut db).unwrap();
    test_table_map(vec![02], &mut db).unwrap();
}

#[test]
fn leveldb_table_map() {
    let mut db = leveldb_database();
    test_table_map(vec![01], &mut db).unwrap();
    test_table_map(vec![02], &mut db).unwrap();
}
