// use tempdir::TempDir;
// use leveldb::options::Options;
// use storage::db::Fork;

// use super::{Map, List, MapTable, MerkleTable, Database, StorageValue, Error, MemoryDB, LevelDB};

// fn leveldb_database() -> LevelDB {
//     let mut options = Options::new();
//     options.create_if_missing = true;
//     LevelDB::new(TempDir::new("da").unwrap().path(), options).unwrap()
// }

// fn test_map_simple<T: Map<[u8], Vec<u8>>>(db: T) -> Result<(), Error> {
//     db.put(b"aba", vec![1, 2, 3])?;
//     assert_eq!(db.get(b"aba")?, Some(vec![1, 2, 3]));
//     assert_eq!(db.get(b"caba")?, None);

//     db.put(b"caba", vec![50, 14])?;
//     db.delete(b"aba")?;
//     assert_eq!(db.get(b"aba")?, None);
//     db.put(b"caba", vec![1, 2, 3, 117, 3])?;
//     assert_eq!(db.get(b"caba")?, Some(vec![1, 2, 3, 117, 3]));
//     Ok(())
// }

// fn test_database_merge<T: Database>(db: T) -> Result<(), Error> {
//     db.put(b"ab", vec![1, 2, 3])?;
//     db.put(b"aba", vec![14, 22, 3])?;
//     db.put(b"caba", vec![34, 2, 3])?;
//     db.put(b"abacaba", vec![1, 65])?;

//     let patch;
//     {
//         let fork = db.fork();

//         fork.put(b"aba", vec![14, 22, 3])?;
//         assert_eq!(b"ab", &fork.find_key(b"").unwrap().unwrap() as &[u8]);

//         fork.delete(b"ab")?;
//         assert_eq!(b"aba", &fork.find_key(b"").unwrap().unwrap() as &[u8]);

//         fork.put(b"aaa", vec![21])?;
//         assert_eq!(b"aaa", &fork.find_key(b"").unwrap().unwrap() as &[u8]);

//         assert_eq!(b"abacaba",
//                    &fork.find_key(b"abac").unwrap().unwrap() as &[u8]);
//         fork.put(b"abacaba", vec![18, 34])?;
//         fork.put(b"caba", vec![10])?;
//         fork.put(b"abac", vec![117, 32, 64])?;
//         fork.put(b"abac", vec![14, 12])?;
//         assert_eq!(b"abac", &fork.find_key(b"abac").unwrap().unwrap() as &[u8]);
//         fork.delete(b"abacaba")?;
//         assert_eq!(b"caba", &fork.find_key(b"abaca").unwrap().unwrap() as &[u8]);


//         assert_eq!(fork.get(b"ab")?, None);
//         assert_eq!(fork.get(b"caba")?, Some(vec![10]));
//         assert_eq!(fork.get(b"abac")?, Some(vec![14, 12]));
//         assert_eq!(fork.get(b"aba")?, Some(vec![14, 22, 3]));
//         assert_eq!(fork.get(b"abacaba")?, None);

//         patch = fork.changes();
//     }
//     assert_eq!(db.get(b"ab")?, Some(vec![1, 2, 3]));
//     assert_eq!(db.get(b"aba")?, Some(vec![14, 22, 3]));
//     assert_eq!(db.get(b"caba")?, Some(vec![34, 2, 3]));
//     assert_eq!(db.get(b"abacaba")?, Some(vec![1, 65]));

//     db.merge(&patch)?;
//     assert_eq!(db.get(b"ab")?, None);
//     assert_eq!(db.get(b"caba")?, Some(vec![10]));
//     assert_eq!(db.get(b"abac")?, Some(vec![14, 12]));
//     assert_eq!(db.get(b"aba")?, Some(vec![14, 22, 3]));
//     assert_eq!(db.get(b"abacaba")?, None);
//     Ok(())
// }

// fn test_table_list<T: Database>(prefix: Vec<u8>, db: &T) -> Result<(), Error> {
//     let list = MerkleTable::new(MapTable::new(prefix, db));
//     assert_eq!(list.len()?, 0 as u64);
//     list.append(vec![10])?;
//     assert_eq!(list.get(0)?, Some(vec![10]));

//     list.append(vec![15])?;
//     assert_eq!(list.len()?, 2);
//     assert_eq!(list.last()?, Some(vec![15]));

//     let bound: u64 = 500;
//     for i in 0..bound {
//         list.append(StorageValue::serialize(i as u64))?;
//     }
//     assert_eq!(list.last()?, Some(StorageValue::serialize(bound - 1)));
//     assert_eq!(list.len()?, 2 + bound);
//     Ok(())
// }

// fn test_table_map<T: Database>(prefix: Vec<u8>, db: &T) -> Result<(), Error> {
//     let map = MapTable::new(prefix, db);
//     test_map_simple(map)
// }

// fn test_map_find_keys<T: Map<[u8], Vec<u8>>>(db: &T) {
//     db.put(b"a", b"12345".to_vec()).unwrap();
//     db.put(b"ab", b"123456".to_vec()).unwrap();
//     db.put(b"ac", b"123457".to_vec()).unwrap();
//     db.put(b"baca", b"1".to_vec()).unwrap();
//     db.put(b"bza", b"2".to_vec()).unwrap();
//     db.put(b"bzac", b"3".to_vec()).unwrap();

//     assert_eq!(db.find_key(b"a").unwrap(), Some(b"a".to_vec()));
//     assert_eq!(db.find_key(&[]).unwrap(), Some(b"a".to_vec()));
//     assert_eq!(db.find_key(b"b").unwrap(), Some(b"baca".to_vec()));
//     assert_eq!(db.find_key(b"c").unwrap(), None);
// }

// fn test_map_table_different_prefixes<T: Database>(db: &T) {
//     {
//         let map2 = MapTable::new(b"abc".to_vec(), db);
//         map2.put(&b"abac".to_vec(), b"12345".to_vec()).unwrap();
//     }
//     let map1 = MapTable::new(b"bcd".to_vec(), db);
//     map1.put(&b"baca".to_vec(), b"1".to_vec()).unwrap();

//     assert_eq!(map1.find_key(&b"abd".to_vec()).unwrap(), None);
// }

// #[test]
// fn serializer() {
//     let a: u32 = 10;
//     let b: u64 = 15;
//     let c: Vec<u8> = vec![10, 15, 24, 2, 1];

//     let a_s = a.serialize();
//     let b_s = b.serialize();
//     let c_s = c.clone().serialize();
//     let c_d: Vec<u8> = StorageValue::deserialize(c_s);
//     assert_eq!(a, StorageValue::deserialize(a_s));
//     assert_eq!(b, StorageValue::deserialize(b_s));
//     assert_eq!(c, c_d);
// }

// #[test]
// fn memory_database_simple() {
//     let db = MemoryDB::new();
//     test_map_simple(db).unwrap();
// }

// #[test]
// fn leveldb_database_simple() {
//     let db = leveldb_database();
//     test_map_simple(db).unwrap();
// }

// #[test]
// fn memory_database_merge() {
//     let db = MemoryDB::new();
//     test_database_merge(db).unwrap();
// }

// #[test]
// fn leveldb_database_merge() {
//     let db = leveldb_database();
//     test_database_merge(db).unwrap();
// }

// #[test]
// fn memorydb_table_list() {
//     let db = MemoryDB::new();
//     test_table_list(vec![01], &db).unwrap();
//     test_table_list(vec![02], &db).unwrap();
// }

// #[test]
// fn leveldb_table_list() {
//     let db = leveldb_database();
//     test_table_list(vec![01], &db).unwrap();
//     test_table_list(vec![02], &db).unwrap();
// }

// #[test]
// fn memorydb_table_map() {
//     let db = MemoryDB::new();
//     test_table_map(vec![01], &db).unwrap();
//     test_table_map(vec![02], &db).unwrap();
// }

// #[test]
// fn leveldb_table_map() {
//     let db = leveldb_database();
//     test_table_map(vec![01], &db).unwrap();
//     test_table_map(vec![02], &db).unwrap();
// }

// #[test]
// fn leveldb_find_key() {
//     let db = leveldb_database();
//     test_map_find_keys(&db);
// }

// #[test]
// fn memorydb_find_key() {
//     let db = MemoryDB::new();
//     test_map_find_keys(&db);
// }

// #[test]
// fn leveldb_map_find_key() {
//     let db = leveldb_database();
//     let map = MapTable::new(vec![02], &db);
//     test_map_find_keys(&map);
// }

// #[test]
// fn memorydb_map_find_key() {
//     let db = MemoryDB::new();
//     let map = MapTable::new(vec![02], &db);
//     test_map_find_keys(&map);
// }

// #[test]
// fn leveldb_map_table_different_prefixes() {
//     let db = leveldb_database();
//     test_map_table_different_prefixes(&db);
// }

// #[test]
// fn memorydb_map_table_different_prefixes() {
//     let db = MemoryDB::new();
//     test_map_table_different_prefixes(&db);
// }


// #[test]
// fn memorydb_iter() {
//     let mut db = MemoryDB::new();
//     db.put(b"a", b"12345".to_vec()).unwrap();
//     db.put(b"ab", b"123456".to_vec()).unwrap();
//     db.put(b"ac", b"123457".to_vec()).unwrap();
//     db.put(b"baca", b"1".to_vec()).unwrap();
//     db.put(b"bza", b"2".to_vec()).unwrap();
//     db.put(b"bzac", b"3".to_vec()).unwrap();

//     let mut it = db.iter();
//     assert_eq!(it.next(), Some((b"a".to_vec(), b"12345".to_vec())));
//     assert_eq!(it.next(), Some((b"ab".to_vec(), b"123456".to_vec())));
//     assert_eq!(it.next(), Some((b"ac".to_vec(), b"123457".to_vec())));

//     assert_eq!(it.seek(&b"bza".to_vec()), Some((b"bza".to_vec(), b"2".to_vec())));
//     assert_eq!(it.next(), Some((b"bzac".to_vec(), b"3".to_vec())));
// }

// #[test]
// fn leveldb_iter() {
//     let mut db = leveldb_database();

//     db.put(b"a", b"12345".to_vec()).unwrap();
//     db.put(b"ab", b"123456".to_vec()).unwrap();
//     db.put(b"ac", b"123457".to_vec()).unwrap();
//     db.put(b"baca", b"1".to_vec()).unwrap();
//     db.put(b"bza", b"2".to_vec()).unwrap();
//     db.put(b"bzac", b"3".to_vec()).unwrap();

//     let mut it = db.iter();
//     assert_eq!(it.next(), Some((b"a".to_vec(), b"12345".to_vec())));
//     assert_eq!(it.next(), Some((b"ab".to_vec(), b"123456".to_vec())));
//     assert_eq!(it.next(), Some((b"ac".to_vec(), b"123457".to_vec())));

//     assert_eq!(it.seek(&b"bza".to_vec()), Some((b"bza".to_vec(), b"2".to_vec())));
//     assert_eq!(it.next(), Some((b"bzac".to_vec(), b"3".to_vec())));
// }

// #[test]
// fn leveldb_map_table_iter() {
//     let mut db = leveldb_database();
//     let mut map = MapTable::new(vec![02], &mut db);

//     map.put(&vec![1, 2], vec![1, 3, 4]).unwrap();
//     map.put(&vec![1, 3], vec![1, 4, 5]).unwrap();
//     map.put(&vec![2, 3], vec![2, 4, 5]).unwrap();
//     map.put(&vec![2, 4], vec![4, 4, 5]).unwrap();

//     let mut it = map.into_iter();
//     assert_eq!(it.next(), Some((vec![1, 2], vec![1, 3, 4])));
//     assert_eq!(it.next(), Some((vec![1, 3], vec![1, 4, 5])));
//     assert_eq!(it.seek(&vec![2, 4]), Some((vec![2, 4], vec![4, 4, 5])));
// }
