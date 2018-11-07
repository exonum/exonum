// Copyright 2018 The Exonum Team
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

use super::{
    Database, Entry, Fork, KeySetIndex, ListIndex, MapIndex, ProofListIndex, ProofMapIndex,
    Snapshot, SparseListIndex, ValueSetIndex,
};
use crypto::Hash;

const IDX_NAME: &'static str = "idx_name";

fn fork_iter<T: Database>(db: T) {
    let mut fork = db.fork();

    fork.put(IDX_NAME, vec![10], vec![10]);
    fork.put(IDX_NAME, vec![20], vec![20]);
    fork.put(IDX_NAME, vec![30], vec![30]);

    assert!(fork.contains(IDX_NAME, &[10]));

    db.merge(fork.into_patch()).unwrap();

    let mut fork = db.fork();

    assert!(fork.contains(IDX_NAME, &[10]));

    fn assert_iter(fork: &Fork, from: u8, assumed: &[(u8, u8)]) {
        let mut values = Vec::new();

        let mut iter = fork.iter(IDX_NAME, &[from]);
        while let Some((k, v)) = iter.next() {
            values.push((k[0], v[0]));
        }
        assert_eq!(values, assumed);
    }

    // Stored
    assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
    assert_iter(&fork, 5, &[(10, 10), (20, 20), (30, 30)]);
    assert_iter(&fork, 10, &[(10, 10), (20, 20), (30, 30)]);
    assert_iter(&fork, 11, &[(20, 20), (30, 30)]);
    assert_iter(&fork, 31, &[]);

    // Inserted
    fork.put(IDX_NAME, vec![5], vec![5]);
    assert_iter(&fork, 0, &[(5, 5), (10, 10), (20, 20), (30, 30)]);
    fork.put(IDX_NAME, vec![25], vec![25]);
    assert_iter(&fork, 0, &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30)]);
    fork.put(IDX_NAME, vec![35], vec![35]);
    assert_iter(
        &fork,
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30), (35, 35)],
    );

    // Double inserted
    fork.put(IDX_NAME, vec![25], vec![23]);
    assert_iter(
        &fork,
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 23), (30, 30), (35, 35)],
    );
    fork.put(IDX_NAME, vec![26], vec![26]);
    assert_iter(
        &fork,
        0,
        &[
            (5, 5),
            (10, 10),
            (20, 20),
            (25, 23),
            (26, 26),
            (30, 30),
            (35, 35),
        ],
    );

    // Replaced
    let mut fork = db.fork();

    fork.put(IDX_NAME, vec![10], vec![11]);
    assert_iter(&fork, 0, &[(10, 11), (20, 20), (30, 30)]);
    fork.put(IDX_NAME, vec![30], vec![31]);
    assert_iter(&fork, 0, &[(10, 11), (20, 20), (30, 31)]);

    // Deleted
    let mut fork = db.fork();

    fork.remove(IDX_NAME, vec![20]);
    assert_iter(&fork, 0, &[(10, 10), (30, 30)]);
    fork.remove(IDX_NAME, vec![10]);
    assert_iter(&fork, 0, &[(30, 30)]);
    fork.put(IDX_NAME, vec![10], vec![11]);
    assert_iter(&fork, 0, &[(10, 11), (30, 30)]);
    fork.remove(IDX_NAME, vec![10]);
    assert_iter(&fork, 0, &[(30, 30)]);

    // MissDeleted
    let mut fork = db.fork();

    fork.remove(IDX_NAME, vec![5]);
    assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
    fork.remove(IDX_NAME, vec![15]);
    assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
    fork.remove(IDX_NAME, vec![35]);
    assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
}

fn changelog<T: Database>(db: T) {
    let mut fork = db.fork();

    fork.put(IDX_NAME, vec![1], vec![1]);
    fork.put(IDX_NAME, vec![2], vec![2]);
    fork.put(IDX_NAME, vec![3], vec![3]);

    assert_eq!(fork.get(IDX_NAME, &[1]), Some(vec![1]));
    assert_eq!(fork.get(IDX_NAME, &[2]), Some(vec![2]));
    assert_eq!(fork.get(IDX_NAME, &[3]), Some(vec![3]));

    fork.checkpoint();

    assert_eq!(fork.get(IDX_NAME, &[1]), Some(vec![1]));
    assert_eq!(fork.get(IDX_NAME, &[2]), Some(vec![2]));
    assert_eq!(fork.get(IDX_NAME, &[3]), Some(vec![3]));

    fork.put(IDX_NAME, vec![1], vec![10]);
    fork.put(IDX_NAME, vec![4], vec![40]);
    fork.remove(IDX_NAME, vec![2]);

    assert_eq!(fork.get(IDX_NAME, &[1]), Some(vec![10]));
    assert_eq!(fork.get(IDX_NAME, &[2]), None);
    assert_eq!(fork.get(IDX_NAME, &[3]), Some(vec![3]));
    assert_eq!(fork.get(IDX_NAME, &[4]), Some(vec![40]));

    fork.rollback();

    assert_eq!(fork.get(IDX_NAME, &[1]), Some(vec![1]));
    assert_eq!(fork.get(IDX_NAME, &[2]), Some(vec![2]));
    assert_eq!(fork.get(IDX_NAME, &[3]), Some(vec![3]));
    assert_eq!(fork.get(IDX_NAME, &[4]), None);

    fork.checkpoint();

    fork.put(IDX_NAME, vec![4], vec![40]);
    fork.put(IDX_NAME, vec![4], vec![41]);
    fork.remove(IDX_NAME, vec![2]);
    fork.put(IDX_NAME, vec![2], vec![20]);

    assert_eq!(fork.get(IDX_NAME, &[1]), Some(vec![1]));
    assert_eq!(fork.get(IDX_NAME, &[2]), Some(vec![20]));
    assert_eq!(fork.get(IDX_NAME, &[3]), Some(vec![3]));
    assert_eq!(fork.get(IDX_NAME, &[4]), Some(vec![41]));

    fork.rollback();

    assert_eq!(fork.get(IDX_NAME, &[1]), Some(vec![1]));
    assert_eq!(fork.get(IDX_NAME, &[2]), Some(vec![2]));
    assert_eq!(fork.get(IDX_NAME, &[3]), Some(vec![3]));
    assert_eq!(fork.get(IDX_NAME, &[4]), None);

    fork.put(IDX_NAME, vec![2], vec![20]);

    fork.checkpoint();

    fork.put(IDX_NAME, vec![3], vec![30]);

    fork.rollback();

    assert_eq!(fork.get(IDX_NAME, &[1]), Some(vec![1]));
    assert_eq!(fork.get(IDX_NAME, &[2]), Some(vec![20]));
    assert_eq!(fork.get(IDX_NAME, &[3]), Some(vec![3]));
    assert_eq!(fork.get(IDX_NAME, &[4]), None);
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
    use super::super::{DbOptions, RocksDB};
    use std::path::Path;
    use storage::{Database, ListIndex, Snapshot};
    use tempdir::TempDir;

    fn rocksdb_database(path: &Path) -> RocksDB {
        let options = DbOptions::default();
        RocksDB::open(path, &options).unwrap()
    }

    #[test]
    fn test_rocksdb_fork_iter() {
        let dir = TempDir::new("exonum_rocksdb1").unwrap();
        let path = dir.path();
        super::fork_iter(rocksdb_database(path));
    }

    #[test]
    fn test_rocksdb_changelog() {
        let dir = TempDir::new("exonum_rocksdb2").unwrap();
        let path = dir.path();
        super::changelog(rocksdb_database(path));
    }

    #[ignore]
    #[test]
    fn test_multiple_patch() {
        let dir = TempDir::new("exonum_rocksdb_multiple_patch").unwrap();
        let path = dir.path();
        let db = rocksdb_database(path);
        fn list_index<View: AsRef<dyn Snapshot>>(view: View) -> ListIndex<View, u64> {
            ListIndex::new("list_index", view)
        }
        // create first patch
        let patch1 = {
            let mut fork = db.fork();
            {
                let mut index = list_index(&mut fork);
                index.push(1);
                index.push(3);
                index.push(4);
            }
            fork.into_patch()
        };
        // create second patch
        let patch2 = {
            let mut fork = db.fork();
            {
                let mut index = list_index(&mut fork);
                index.push(2);
            }
            fork.into_patch()
        };
        db.merge(patch1).unwrap();
        db.merge(patch2).unwrap();
        let snapshot = db.snapshot();
        let index = list_index(snapshot);
        let iter = index.iter();
        assert_eq!(index.len() as usize, iter.count());
    }
}

// This should compile to ensure ?Sized bound on `new_in_family` (see #1024).
#[allow(dead_code, unreachable_code, unused_variables)]
fn should_compile() {
    let mut fork: Fork = unimplemented!();
    let _: Entry<_, ()> = Entry::new_in_family("", "", &mut fork);
    let _: KeySetIndex<_, Hash> = KeySetIndex::new_in_family("", "", &mut fork);
    let _: ListIndex<_, ()> = ListIndex::new_in_family("", "", &mut fork);
    let _: MapIndex<_, Hash, ()> = MapIndex::new_in_family("", "", &mut fork);
    let _: ProofListIndex<_, ()> = ProofListIndex::new_in_family("", "", &mut fork);
    let _: ProofMapIndex<_, Hash, ()> = ProofMapIndex::new_in_family("", "", &mut fork);
    let _: SparseListIndex<_, ()> = SparseListIndex::new_in_family("", "", &mut fork);
    let _: ValueSetIndex<_, ()> = ValueSetIndex::new_in_family("", "", &mut fork);
}
