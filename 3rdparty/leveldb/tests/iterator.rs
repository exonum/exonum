use utils::{open_database,tmpdir,db_put_simple};
use leveldb::iterator::Iterable;
use leveldb::options::ReadOptions;

#[test]
fn test_iterator() {
  let tmp = tmpdir("iter");
  let database = &mut open_database(tmp.path(), true);
  db_put_simple(database, [1], &[1]);
  db_put_simple(database, [2], &[2]);

  let read_opts = ReadOptions::new();
  let mut iter = database.iter(read_opts);

  {
    let entry = iter.next();
    assert!(entry.is_some());
    assert_eq!(entry.unwrap(), ([1].to_vec().as_slice(), [1].to_vec().as_slice()));
  }
  {
    let entry2 = iter.next();
    assert!(entry2.is_some());
    assert_eq!(entry2.unwrap(), ([2].to_vec().as_slice(), [2].to_vec().as_slice()));
  }
  assert!(iter.next().is_none());
}

#[test]
fn test_iterator_last() {
  let tmp = tmpdir("iter_last");
  let database = &mut open_database(tmp.path(), true);
  db_put_simple(database, [1], &[1]);
  db_put_simple(database, [2], &[2]);

  let read_opts = ReadOptions::new();
  let mut iter = database.iter(read_opts);

  assert!(iter.next().is_some());
}

#[test]
fn test_iterator_from_to() {
  let tmp = tmpdir("from_to");
  let database = &mut open_database(tmp.path(), true);
  db_put_simple(database, [1], &[1]);
  db_put_simple(database, [2], &[2]);
  db_put_simple(database, [3], &[3]);
  db_put_simple(database, [4], &[4]);
  db_put_simple(database, [5], &[5]);

  let read_opts = ReadOptions::new();
  let mut iter = database.iter(read_opts);

  assert_eq!(iter.next().unwrap(), ([1].to_vec().as_slice(), vec![1].as_slice()));
}


#[test]
fn test_key_iterator() {
  let tmp = tmpdir("key_iter");
  let database = &mut open_database(tmp.path(), true);
  db_put_simple(database, [1], &[1]);
  db_put_simple(database, [2], &[2]);

  let iterable: &mut Iterable = database;

  let read_opts = ReadOptions::new();
  let mut iter = iterable.iter(read_opts);
  let (key, _) = iter.next().unwrap();
  assert_eq!(key, vec![1].as_slice());
}

#[test]
fn test_value_iterator() {
  let tmp = tmpdir("value_iter");
  let database = &mut open_database(tmp.path(), true);
  db_put_simple(database, [1], &[1]);
  db_put_simple(database, [2], &[2]);

  let iterable: &mut Iterable = database;

  let read_opts = ReadOptions::new();
  let mut iter = iterable.iter(read_opts);
  let (_, value) = iter.next().unwrap();
  assert_eq!(value, vec![1].as_slice());
}
