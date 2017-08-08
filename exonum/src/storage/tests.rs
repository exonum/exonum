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

use tempdir::TempDir;

use super::{Database, LevelDB, LevelDBOptions, MemoryDB, Snapshot, Fork};


fn leveldb_database() -> LevelDB {
    let mut options = LevelDBOptions::new();
    options.create_if_missing = true;
    LevelDB::open(TempDir::new("exonum").unwrap().path(), options).unwrap()
}

fn memorydb_database() -> MemoryDB {
    MemoryDB::new()
}

fn fork_iter<T: Database>(mut db: T) {
    let mut fork = db.fork();

    fork.put(vec![10], vec![10]);
    fork.put(vec![20], vec![20]);
    fork.put(vec![30], vec![30]);

    db.merge(fork.into_patch()).unwrap();

    let mut fork = db.fork();

    fn assert_iter(fork: &Fork, from: u8, assumed: &[(u8, u8)]) {
        let mut values = Vec::new();
        let mut iter = fork.iter(&[from]);
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
    fork.put(vec![5], vec![5]);
    assert_iter(&fork, 0, &[(5, 5), (10, 10), (20, 20), (30, 30)]);
    fork.put(vec![25], vec![25]);
    assert_iter(&fork, 0, &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30)]);
    fork.put(vec![35], vec![35]);
    assert_iter(
        &fork,
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30), (35, 35)],
    );

    // Double inserted
    fork.put(vec![25], vec![23]);
    assert_iter(
        &fork,
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 23), (30, 30), (35, 35)],
    );
    fork.put(vec![26], vec![26]);
    assert_iter(
        &fork,
        0,
        &[(5, 5), (10, 10), (20, 20), (25, 23), (26, 26), (30, 30), (35, 35)],
    );

    // Replaced
    let mut fork = db.fork();
    fork.put(vec![10], vec![11]);
    assert_iter(&fork, 0, &[(10, 11), (20, 20), (30, 30)]);
    fork.put(vec![30], vec![31]);
    assert_iter(&fork, 0, &[(10, 11), (20, 20), (30, 31)]);

    // Deleted
    let mut fork = db.fork();
    fork.remove(vec![20]);
    assert_iter(&fork, 0, &[(10, 10), (30, 30)]);
    fork.remove(vec![10]);
    assert_iter(&fork, 0, &[(30, 30)]);
    fork.put(vec![10], vec![11]);
    assert_iter(&fork, 0, &[(10, 11), (30, 30)]);
    fork.remove(vec![10]);
    assert_iter(&fork, 0, &[(30, 30)]);

    // MissDeleted
    let mut fork = db.fork();
    fork.remove(vec![5]);
    assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
    fork.remove(vec![15]);
    assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
    fork.remove(vec![35]);
    assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
}

fn changelog<T: Database>(db: T) {
    let mut fork = db.fork();

    fork.put(vec![1], vec![1]);
    fork.put(vec![2], vec![2]);
    fork.put(vec![3], vec![3]);

    assert_eq!(fork.get(&[1]), Some(vec![1]));
    assert_eq!(fork.get(&[2]), Some(vec![2]));
    assert_eq!(fork.get(&[3]), Some(vec![3]));

    fork.checkpoint();

    assert_eq!(fork.get(&[1]), Some(vec![1]));
    assert_eq!(fork.get(&[2]), Some(vec![2]));
    assert_eq!(fork.get(&[3]), Some(vec![3]));

    fork.put(vec![1], vec![10]);
    fork.put(vec![4], vec![40]);
    fork.remove(vec![2]);

    assert_eq!(fork.get(&[1]), Some(vec![10]));
    assert_eq!(fork.get(&[2]), None);
    assert_eq!(fork.get(&[3]), Some(vec![3]));
    assert_eq!(fork.get(&[4]), Some(vec![40]));

    fork.rollback();

    assert_eq!(fork.get(&[1]), Some(vec![1]));
    assert_eq!(fork.get(&[2]), Some(vec![2]));
    assert_eq!(fork.get(&[3]), Some(vec![3]));
    assert_eq!(fork.get(&[4]), None);

    fork.checkpoint();

    fork.put(vec![4], vec![40]);
    fork.put(vec![4], vec![41]);
    fork.remove(vec![2]);
    fork.put(vec![2], vec![20]);

    assert_eq!(fork.get(&[1]), Some(vec![1]));
    assert_eq!(fork.get(&[2]), Some(vec![20]));
    assert_eq!(fork.get(&[3]), Some(vec![3]));
    assert_eq!(fork.get(&[4]), Some(vec![41]));

    fork.rollback();

    assert_eq!(fork.get(&[1]), Some(vec![1]));
    assert_eq!(fork.get(&[2]), Some(vec![2]));
    assert_eq!(fork.get(&[3]), Some(vec![3]));
    assert_eq!(fork.get(&[4]), None);

    fork.put(vec![2], vec![20]);

    fork.checkpoint();

    fork.put(vec![3], vec![30]);

    fork.rollback();

    assert_eq!(fork.get(&[1]), Some(vec![1]));
    assert_eq!(fork.get(&[2]), Some(vec![20]));
    assert_eq!(fork.get(&[3]), Some(vec![3]));
    assert_eq!(fork.get(&[4]), None);
}

#[test]
fn test_leveldb_fork_iter() {
    fork_iter(leveldb_database())
}

#[test]
fn test_memory_fork_iter() {
    fork_iter(memorydb_database())
}

#[test]
fn test_leveldb_changelog() {
    changelog(leveldb_database())
}

#[test]
fn test_memory_changelog() {
    changelog(memorydb_database())
}

#[test]
fn test_shared_state_memorydb() {
    let mut db = MemoryDB::new();
    let db_clone = Database::clone(&db);

    {
        let mut fork = db.fork();
        fork.put(vec![1, 2, 3], vec![2, 3, 4]);
        db.merge(fork.into_patch()).unwrap();
    }

    assert_eq!(db.fork().get(&[1, 2, 3]), Some(vec![2, 3, 4]));
    assert_eq!(db_clone.fork().get(&[1, 2, 3]), Some(vec![2, 3, 4]));
}