use tempdir::TempDir;
use leveldb::options::Options;
use storage::db::Fork;

use super::{Map, List, MapTable, MerkleTable, Database, StorageValue, Error, MemoryDB, LevelDB,
            U64Key};

fn leveldb_database() -> LevelDB {
    let mut options = Options::new();
    options.create_if_missing = true;
    LevelDB::new(TempDir::new("da").unwrap().path(), options).unwrap()
}

fn test_map_simple<T: Map<[u8], Vec<u8>>>(db: T) -> Result<(), Error> {
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

fn test_database_merge<T: Database>(db: T) -> Result<(), Error> {
    db.put(b"ab", vec![1, 2, 3])?;
    db.put(b"aba", vec![14, 22, 3])?;
    db.put(b"caba", vec![34, 2, 3])?;
    db.put(b"abacaba", vec![1, 65])?;

    let patch;
    {
        let fork = db.fork();

        fork.put(b"aba", vec![14, 22, 3])?;
        assert_eq!(b"ab", &fork.find_key(b"").unwrap().unwrap() as &[u8]);

        fork.delete(b"ab")?;
        assert_eq!(b"aba", &fork.find_key(b"").unwrap().unwrap() as &[u8]);

        fork.put(b"aaa", vec![21])?;
        assert_eq!(b"aaa", &fork.find_key(b"").unwrap().unwrap() as &[u8]);

        assert_eq!(b"abacaba",
                   &fork.find_key(b"abac").unwrap().unwrap() as &[u8]);
        fork.put(b"abacaba", vec![18, 34])?;
        fork.put(b"caba", vec![10])?;
        fork.put(b"abac", vec![117, 32, 64])?;
        fork.put(b"abac", vec![14, 12])?;
        assert_eq!(b"abac", &fork.find_key(b"abac").unwrap().unwrap() as &[u8]);
        fork.delete(b"abacaba")?;
        assert_eq!(b"caba", &fork.find_key(b"abaca").unwrap().unwrap() as &[u8]);


        assert_eq!(fork.get(b"ab")?, None);
        assert_eq!(fork.get(b"caba")?, Some(vec![10]));
        assert_eq!(fork.get(b"abac")?, Some(vec![14, 12]));
        assert_eq!(fork.get(b"aba")?, Some(vec![14, 22, 3]));
        assert_eq!(fork.get(b"abacaba")?, None);

        patch = fork.changes();
    }
    assert_eq!(db.get(b"ab")?, Some(vec![1, 2, 3]));
    assert_eq!(db.get(b"aba")?, Some(vec![14, 22, 3]));
    assert_eq!(db.get(b"caba")?, Some(vec![34, 2, 3]));
    assert_eq!(db.get(b"abacaba")?, Some(vec![1, 65]));

    db.merge(&patch)?;
    assert_eq!(db.get(b"ab")?, None);
    assert_eq!(db.get(b"caba")?, Some(vec![10]));
    assert_eq!(db.get(b"abac")?, Some(vec![14, 12]));
    assert_eq!(db.get(b"aba")?, Some(vec![14, 22, 3]));
    assert_eq!(db.get(b"abacaba")?, None);
    Ok(())
}

fn test_table_list<T: Database>(prefix: Vec<u8>, db: &T) -> Result<(), Error> {
    let list = MerkleTable::new(MapTable::new(prefix, db));
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

fn test_table_map<T: Database>(prefix: Vec<u8>, db: &T) -> Result<(), Error> {
    let map = MapTable::new(prefix, db);
    test_map_simple(map)
}

fn test_map_find_keys<T: Map<[u8], Vec<u8>>>(db: &T) {
    db.put(b"a", b"12345".to_vec()).unwrap();
    db.put(b"ab", b"123456".to_vec()).unwrap();
    db.put(b"ac", b"123457".to_vec()).unwrap();
    db.put(b"baca", b"1".to_vec()).unwrap();
    db.put(b"bza", b"2".to_vec()).unwrap();
    db.put(b"bzac", b"3".to_vec()).unwrap();

    assert_eq!(db.find_key(b"a").unwrap(), Some(b"a".to_vec()));
    assert_eq!(db.find_key(&[]).unwrap(), Some(b"a".to_vec()));
    assert_eq!(db.find_key(b"b").unwrap(), Some(b"baca".to_vec()));
    assert_eq!(db.find_key(b"c").unwrap(), None);
}

fn test_map_table_different_prefixes<T: Database>(db: &T) {
    {
        let map2 = MapTable::new(b"abc".to_vec(), db);
        map2.put(&b"abac".to_vec(), b"12345".to_vec()).unwrap();
    }
    let map1 = MapTable::new(b"bcd".to_vec(), db);
    map1.put(&b"baca".to_vec(), b"1".to_vec()).unwrap();

    assert_eq!(map1.find_key(&b"abd".to_vec()).unwrap(),
               Some(b"baca".to_vec()));
}

fn test_map_table_number_keys_find<T: Database>(db: &T) {
    let find_u64_key = |map: &Map<U64Key, Vec<u8>>, key: u64| {
        map.find_key(&U64Key::from(key))
            .unwrap()
            .map(|x| u64::from(U64Key::from_vec(x)))
    };

    let map = MapTable::new(b"abacd".to_vec(), db);
    map.put(&U64Key::from(100), b"1".to_vec()).unwrap();
    map.put(&U64Key::from(110), b"12".to_vec()).unwrap();
    map.put(&U64Key::from(1100), b"123".to_vec()).unwrap();
    map.put(&U64Key::from(500), b"1234".to_vec()).unwrap();
    map.put(&U64Key::from(9000), b"12345".to_vec()).unwrap();

    assert_eq!(find_u64_key(&map, 0), Some(100));
    assert_eq!(find_u64_key(&map, 100), Some(100));
    assert_eq!(find_u64_key(&map, 101), Some(110));
    assert_eq!(find_u64_key(&map, 111), Some(500));
    assert_eq!(find_u64_key(&map, 501), Some(1100));
    assert_eq!(find_u64_key(&map, 1200), Some(9000));
    assert_eq!(find_u64_key(&map, 10000), None);
}


fn test_map_table_number_keys_find_fork<T>(db: &T) 
    where T: Database 
{
    let find_u64_key = |map: &Map<[u8], Vec<u8>>, key: u64| {
        map.find_key(U64Key::from(key).as_ref())
            .unwrap()
            .map(|x| u64::from(U64Key::from_vec(x)))
    };

    db.put(U64Key::from(100).as_ref(), b"1".to_vec()).unwrap();
    db.put(U64Key::from(110).as_ref(), b"12".to_vec()).unwrap();
    db.put(U64Key::from(1100).as_ref(), b"123".to_vec())
        .unwrap();

    let patch = {
        let fork = db.fork();

        fork.put(U64Key::from(500).as_ref(), b"1234".to_vec())
            .unwrap();
        fork.put(U64Key::from(9000).as_ref(), b"12345".to_vec())
            .unwrap();
        fork.delete(U64Key::from(110).as_ref()).unwrap();

        assert_eq!(find_u64_key(&fork, 0), Some(100));
        assert_eq!(find_u64_key(&fork, 101), Some(500));
        assert_eq!(find_u64_key(&fork, 111), Some(500));
        assert_eq!(find_u64_key(&fork, 501), Some(1100));
        assert_eq!(find_u64_key(&fork, 1200), Some(9000));
        assert_eq!(find_u64_key(&fork, 10000), None);

        fork.changes()
    };

    assert_eq!(find_u64_key(db, 0), Some(100));
    assert_eq!(find_u64_key(db, 100), Some(100));
    assert_eq!(find_u64_key(db, 101), Some(110));
    assert_eq!(find_u64_key(db, 111), Some(1100));
    assert_eq!(find_u64_key(db, 501), Some(1100));
    assert_eq!(find_u64_key(db, 1200), None);

    db.merge(&patch).unwrap();

    assert_eq!(find_u64_key(db, 0), Some(100));
    assert_eq!(find_u64_key(db, 101), Some(500));
    assert_eq!(find_u64_key(db, 111), Some(500));
    assert_eq!(find_u64_key(db, 501), Some(1100));
    assert_eq!(find_u64_key(db, 1200), Some(9000));
    assert_eq!(find_u64_key(db, 10000), None);
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
    let a_d: u32 = StorageValue::deserialize(a_s);
    let b_d: u64 = StorageValue::deserialize(b_s);
    assert_eq!(a, a_d);
    assert_eq!(b, b_d);
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
    let db = MemoryDB::new();
    test_table_list(vec![01], &db).unwrap();
    test_table_list(vec![02], &db).unwrap();
}

#[test]
fn leveldb_table_list() {
    let db = leveldb_database();
    test_table_list(vec![01], &db).unwrap();
    test_table_list(vec![02], &db).unwrap();
}

#[test]
fn memorydb_table_map() {
    let db = MemoryDB::new();
    test_table_map(vec![01], &db).unwrap();
    test_table_map(vec![02], &db).unwrap();
}

#[test]
fn leveldb_table_map() {
    let db = leveldb_database();
    test_table_map(vec![01], &db).unwrap();
    test_table_map(vec![02], &db).unwrap();
}

#[test]
fn leveldb_find_key() {
    let db = leveldb_database();
    test_map_find_keys(&db);
}

#[test]
fn memorydb_find_key() {
    let db = MemoryDB::new();
    test_map_find_keys(&db);
}

#[test]
fn leveldb_map_find_key() {
    let db = leveldb_database();
    let map = MapTable::new(vec![02], &db);
    test_map_find_keys(&map);
}

#[test]
fn memorydb_map_find_key() {
    let db = MemoryDB::new();
    let map = MapTable::new(vec![02], &db);
    test_map_find_keys(&map);
}

#[test]
fn leveldb_map_table_different_prefixes() {
    let db = leveldb_database();
    test_map_table_different_prefixes(&db);
}

#[test]
fn memorydb_map_table_different_prefixes() {
    let db = MemoryDB::new();
    test_map_table_different_prefixes(&db);
}

#[test]
fn leveldb_map_table_number_keys_find() {
    let db = leveldb_database();
    test_map_table_number_keys_find(&db);
}

#[test]
fn memorydb_map_table_number_keys_find() {
    let db = MemoryDB::new();
    test_map_table_number_keys_find(&db);
}

#[test]
fn leveldb_map_table_number_keys_find_fork() {
    let db = leveldb_database();
    test_map_table_number_keys_find_fork(&db);
}

#[test]
fn memorydb_map_table_number_keys_find_fork() {
    let db = MemoryDB::new();
    test_map_table_number_keys_find_fork(&db);
}


// TODO add tests for changes
