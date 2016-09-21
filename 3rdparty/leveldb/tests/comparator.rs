#[cfg(test)]
mod comparator {
  use libc::c_char;
  use utils::{tmpdir, db_put_simple};
  use leveldb::database::{Database};
  use leveldb::iterator::Iterable;
  use leveldb::options::{Options,ReadOptions};
  use leveldb::comparator::{Comparator,OrdComparator};
  use std::cmp::Ordering;
  
  struct ReverseComparator {}

  impl Comparator for ReverseComparator {

    fn name(&self) -> *const c_char {
      "reverse".as_ptr() as *const c_char
    }
  
    fn compare(&self, a: &[u8], b: &[u8]) -> Ordering {
      b.cmp(a)
    }
  }

  #[test]
  fn test_comparator() {
    let comparator: ReverseComparator = ReverseComparator {};
    let mut opts = Options::new();
    opts.create_if_missing = true;
    let tmp = tmpdir("reverse_comparator");
    let database = &mut Database::open_with_comparator(tmp.path(), opts, comparator).unwrap();
    db_put_simple(database, b"1", &[1]);
    db_put_simple(database, b"2", &[2]);

    let read_opts = ReadOptions::new();
    let mut iter = database.iter(read_opts);

    assert_eq!((b"2".to_vec().as_slice(), vec![2]), iter.next().unwrap());
    assert_eq!((b"1".to_vec().as_slice(), vec![1]), iter.next().unwrap());
  }

  #[test]
  fn test_ord_comparator() {
    let comparator: OrdComparator = OrdComparator::new("foo");
    let mut opts = Options::new();
    opts.create_if_missing = true;
    let tmp = tmpdir("ord_comparator");
    let database = &mut Database::open_with_comparator(tmp.path(), opts, comparator).unwrap();
    db_put_simple(database, b"1", &[1]);
    db_put_simple(database, b"2", &[2]);

    let read_opts = ReadOptions::new();
    let mut iter = database.iter(read_opts);

    assert_eq!((b"1".to_vec().as_slice(), vec![1]), iter.next().unwrap());
    assert_eq!((b"2".to_vec().as_slice(), vec![2]), iter.next().unwrap());
  }
}
