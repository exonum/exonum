// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::{Database, View};

fn fork_iter<T: Database>(db: T) {
    let fork = db.fork();

    fork.put("a", &[10], &[10]);
    fork.put("a", &[20], &[20]);
    fork.put("a", &[30], &[30]);

    fork.commit();

    fn assert_iter(fork: &View, from: u8, assumed: &[(u8, u8)]) {
        let mut values = Vec::new();
        let mut iter = fork.iter("a", Some(&[from]));
        while let Some((k, v)) = iter.next() {
            values.push((k[0], v[0]));
        }
        assert_eq!(values, assumed);
    }

    // Stored
    assert_iter(fork.as_ref(), 0, &[(10, 10), (20, 20), (30, 30)]);
    assert_iter(fork.as_ref(), 5, &[(10, 10), (20, 20), (30, 30)]);
    assert_iter(fork.as_ref(), 10, &[(10, 10), (20, 20), (30, 30)]);
    assert_iter(fork.as_ref(), 11, &[(20, 20), (30, 30)]);
    assert_iter(fork.as_ref(), 31, &[]);

    // Inserted
    fork.put("a", &[5], &[5]);
    assert_iter(fork.as_ref(), 0, &[(5, 5), (10, 10), (20, 20), (30, 30)]);
    fork.put("a", &[25], &[25]);
    assert_iter(
        fork.as_ref(),
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30)],
    );
    fork.put("a", &[35], &[35]);
    assert_iter(
        fork.as_ref(),
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30), (35, 35)],
    );

    // Double inserted
    fork.put("a", &[25], &[23]);
    assert_iter(
        fork.as_ref(),
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 23), (30, 30), (35, 35)],
    );
    fork.put("a", &[26], &[26]);
    assert_iter(
        fork.as_ref(),
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 23), (26, 26), (30, 30), (35, 35)],
    );

    // Replaced
    let fork = db.fork();
    fork.put("a", &[10], &[11]);
    assert_iter(fork.as_ref(), 0, &[(10, 11), (20, 20), (30, 30)]);
    fork.put("a", &[30], &[31]);
    assert_iter(fork.as_ref(), 0, &[(10, 11), (20, 20), (30, 31)]);

    // Deleted
    let fork = db.fork();
    fork.delete("a", &[20]);
    assert_iter(fork.as_ref(), 0, &[(10, 10), (30, 30)]);
    fork.delete("a", &[10]);
    assert_iter(fork.as_ref(), 0, &[(30, 30)]);
    fork.put("a", &[10], &[11]);
    assert_iter(fork.as_ref(), 0, &[(10, 11), (30, 30)]);
    fork.delete("a", &[10]);
    assert_iter(fork.as_ref(), 0, &[(30, 30)]);

    // MissDeleted
    let fork = db.fork();
    fork.delete("a", &[5]);
    assert_iter(fork.as_ref(), 0, &[(10, 10), (20, 20), (30, 30)]);
    fork.delete("a", &[15]);
    assert_iter(fork.as_ref(), 0, &[(10, 10), (20, 20), (30, 30)]);
    fork.delete("a", &[35]);
    assert_iter(fork.as_ref(), 0, &[(10, 10), (20, 20), (30, 30)]);
}

fn changelog<T: Database>(db: T) {
    let fork = db.fork();

    fork.put("a", &[1], &[1]);
    fork.put("a", &[2], &[2]);
    fork.put("a", &[3], &[3]);

    assert_eq!(fork.get("a", &[1]), Some(vec![1]));
    assert_eq!(fork.get("a", &[2]), Some(vec![2]));
    assert_eq!(fork.get("a", &[3]), Some(vec![3]));

    fork.savepoint();

    assert_eq!(fork.get("a", &[1]), Some(vec![1]));
    assert_eq!(fork.get("a", &[2]), Some(vec![2]));
    assert_eq!(fork.get("a", &[3]), Some(vec![3]));

    fork.put("a", &[1], &[10]);
    fork.put("a", &[4], &[40]);
    fork.delete("a", &[2]);

    assert_eq!(fork.get("a", &[1]), Some(vec![10]));
    assert_eq!(fork.get("a", &[2]), None);
    assert_eq!(fork.get("a", &[3]), Some(vec![3]));
    assert_eq!(fork.get("a", &[4]), Some(vec![40]));

    fork.rollback_to_savepoint();

    assert_eq!(fork.get("a", &[1]), Some(vec![1]));
    assert_eq!(fork.get("a", &[2]), Some(vec![2]));
    assert_eq!(fork.get("a", &[3]), Some(vec![3]));
    assert_eq!(fork.get("a", &[4]), None);

    fork.savepoint();

    fork.put("a", &[4], &[40]);
    fork.put("a", &[4], &[41]);
    fork.delete("a", &[2]);
    fork.put("a", &[2], &[20]);

    assert_eq!(fork.get("a", &[1]), Some(vec![1]));
    assert_eq!(fork.get("a", &[2]), Some(vec![20]));
    assert_eq!(fork.get("a", &[3]), Some(vec![3]));
    assert_eq!(fork.get("a", &[4]), Some(vec![41]));

    fork.rollback_to_savepoint();

    assert_eq!(fork.get("a", &[1]), Some(vec![1]));
    assert_eq!(fork.get("a", &[2]), Some(vec![2]));
    assert_eq!(fork.get("a", &[3]), Some(vec![3]));
    assert_eq!(fork.get("a", &[4]), None);

    fork.put("a", &[2], &[20]);

    fork.savepoint();

    fork.put("a", &[3], &[30]);

    fork.rollback_to_savepoint();

    assert_eq!(fork.get("a", &[1]), Some(vec![1]));
    assert_eq!(fork.get("a", &[2]), Some(vec![20]));
    assert_eq!(fork.get("a", &[3]), Some(vec![3]));
    assert_eq!(fork.get("a", &[4]), None);
}


mod memorydb_tests {
    use super::super::MemoryDB;

    fn memorydb_database() -> MemoryDB {
        MemoryDB::new()
    }

    #[test]
    fn test_memory_fork_iter() {
        super::fork_iter(memorydb_database());
    }

    #[test]
    fn test_memory_changelog() {
        super::changelog(memorydb_database());
    }
}

mod rocksdb_tests {
    use std::path::Path;
    use tempdir::TempDir;
    use rand::{Rng, thread_rng};
    use super::super::{Database, RocksDB, RocksDBOptions};

    fn gen_tempdir_name() -> String {
        thread_rng().gen_ascii_chars().take(20).collect()
    }

    fn rocksdb_database(path: &Path) -> RocksDB {
        let mut options = RocksDBOptions::default();
        options.create_if_missing(true);
        RocksDB::open(path, options).unwrap()
    }

    #[test]
    fn test_rocksdb_fork_iter() {
        let dir = TempDir::new(gen_tempdir_name().as_ref()).unwrap();
        let path = dir.path();
        super::fork_iter(rocksdb_database(path));
    }

    #[test]
    fn test_rocksdb_changelog() {
        let dir = TempDir::new(gen_tempdir_name().as_ref()).unwrap();
        let path = dir.path();
        super::changelog(rocksdb_database(path));
    }

    #[test]
    fn test_rocksdb_transaction_commit() {
        let dir = TempDir::new(gen_tempdir_name().as_ref()).unwrap();
        let path = dir.path();
        let db = rocksdb_database(path);
        {
            let txn = db.fork();
            assert!(txn.get("a", b"123").is_none());
            txn.put("a", b"123", b"234");
            assert!(txn.get("a", b"123").is_some());

            let snap = db.snapshot();
            assert!(!snap.contains("a", b"123"));
            txn.commit();
        }

        let snap = db.snapshot();
        assert!(snap.contains("a", b"123"));
    }

    #[test]
    fn test_rocksdb_transaction_rollback() {
        let dir = TempDir::new(gen_tempdir_name().as_ref()).unwrap();
        let path = dir.path();
        let db = rocksdb_database(path);

        {
            let txn = db.fork();
            txn.put("a", b"123", b"234");
            assert!(txn.get("a", b"123").is_some());
            txn.rollback();
        }

        let snapshot = db.snapshot();
        assert!(!snapshot.contains("a", b"123"));
    }

    #[test]
    fn test_rocksdb_transaction_isolation() {
        let dir = TempDir::new(gen_tempdir_name().as_ref()).unwrap();
        let path = dir.path();
        let db = rocksdb_database(path);
        let txn1 = db.fork();
        let txn2 = db.fork();

        txn1.put("a", b"123", b"234");
        assert!(txn1.get("a", b"123").is_some());
        assert!(txn2.get("a", b"123").is_none());
        txn1.commit();
        assert!(txn2.get("a", b"123").is_some());
    }

    #[test]
    fn test_rocksdb_transaction_iter() {
        let dir = TempDir::new(gen_tempdir_name().as_ref()).unwrap();
        let path = dir.path();
        let db = rocksdb_database(path);
        let txn = db.fork();

        txn.put("a", &[1], &[1]);
        txn.put("a", &[2], &[2]);
        txn.put("a", &[3], &[3]);

        let mut iter = txn.iter("a", Some(&[1]));

        assert_eq!(iter.next(), Some((vec![1].as_slice(), vec![1].as_slice())));
        assert_eq!(iter.next(), Some((vec![2].as_slice(), vec![2].as_slice())));
        assert_eq!(iter.next(), Some((vec![3].as_slice(), vec![3].as_slice())));
        assert_eq!(iter.next(), None);

    }
}
