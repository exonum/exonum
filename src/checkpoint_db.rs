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

use std::sync::{Arc, RwLock};

use exonum::storage::{Database, Patch, Result as StorageResult, Snapshot};

/// Implementation of a `Database`, which allows to rollback commits introduced by the `merge()`
/// function.
///
/// **Note:** Intended for testing purposes only. Probably inefficient.
#[derive(Clone, Debug)]
pub struct CheckpointDb<T> {
    inner: T,
    journal: Arc<RwLock<Vec<Patch>>>,
}

impl<T: Database + Clone> CheckpointDb<T> {
    /// Creates a new checkpointed database that uses the specified `db` as the underlying
    /// data storage.
    pub fn new(db: T) -> Self {
        CheckpointDb {
            inner: db,
            journal: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Rolls back this database by udoing the latest `count` `merge()` operations.
    ///
    /// # Panics
    ///
    /// - Panics if more operations are attempted to be reverted than the number of operations
    ///   in the DB journal.
    pub fn rollback(&mut self, count: usize) -> bool {
        let journal_len = self.journal
            .read()
            .expect("Cannot acquire read lock on journal")
            .len();
        assert!(
            journal_len >= count,
            "Cannot rollback {} changes; only {} checkpoints in the journal",
            count,
            journal_len
        );

        let mut journal = self.journal.write().expect(
            "Cannot acquire write lock on journal",
        );

        for _ in 0..count {
            if let Some(patch) = journal.pop() {
                self.inner.merge(patch).expect(
                    "Cannot merge roll-back patch",
                );
            } else {
                panic!("Cannot rollback the database, the journal is empty");
            }
        }

        journal_len < count
    }

    /// Returns a handler to the database. The handler could be used to roll the database back
    /// without having the ownership to it.
    pub fn handler(&self) -> CheckpointDbHandler<T> {
        CheckpointDbHandler(Clone::clone(self))
    }
}

impl<T: Database + Clone> Database for CheckpointDb<T> {
    fn clone(&self) -> Box<Database> {
        Box::new(Clone::clone(self))
    }

    fn snapshot(&self) -> Box<Snapshot> {
        self.inner.snapshot()
    }

    fn merge(&mut self, patch: Patch) -> StorageResult<()> {
        let snapshot = self.inner.snapshot();
        self.inner.merge(patch.clone())?;
        let mut rev_fork = self.fork();

        for (name, changes) in patch {
            for (key, _) in changes {
                match snapshot.get(&name, &key) {
                    Some(value) => {
                        rev_fork.put(&name, key, value);
                    }
                    None => {
                        rev_fork.remove(&name, key);
                    }
                }
            }
        }

        {
            let mut journal = self.journal.write().expect(
                "Cannot acquire write lock on journal",
            );
            journal.push(rev_fork.into_patch());
        }
        Ok(())
    }

    fn merge_sync(&mut self, patch: Patch) -> StorageResult<()> {
        self.merge(patch)
    }
}

/// Handler to a checkpointed database.
#[derive(Clone, Debug)]
pub struct CheckpointDbHandler<T>(CheckpointDb<T>);

impl<T: Database + Clone> CheckpointDbHandler<T> {
    /// Rolls back this database by udoing the latest `count` `merge()` operations.
    pub fn rollback(&mut self, count: usize) -> bool {
        self.0.rollback(count)
    }
}

#[cfg(test)]
mod tests {
    use exonum::storage::{Change, MemoryDB};
    use super::*;

    // Same as `Change`, but with trait implementations required for `Patch` comparison.
    #[derive(Debug, PartialOrd, Ord, PartialEq, Eq)]
    enum OrdChange {
        Put(Vec<u8>),
        Delete,
    }

    impl From<Change> for OrdChange {
        fn from(change: Change) -> Self {
            match change {
                Change::Put(value) => OrdChange::Put(value),
                Change::Delete => OrdChange::Delete,
            }
        }
    }

    impl<'a> From<&'a Change> for OrdChange {
        fn from(change: &'a Change) -> Self {
            match *change {
                Change::Put(ref value) => OrdChange::Put(value.clone()),
                Change::Delete => OrdChange::Delete,
            }
        }
    }

    /// Asserts that a patch contains only the specified changes.
    fn check_patch<'a, I>(patch: &Patch, changes: I)
    where
        I: IntoIterator<Item = (&'a str, Vec<u8>, Change)>,
    {
        use std::collections::BTreeSet;
        use std::iter::FromIterator;

        let mut patch_set: BTreeSet<(&str, _, _)> = BTreeSet::new();
        for (name, changes) in patch.iter() {
            for (key, value) in changes.iter() {
                patch_set.insert((name, key.clone(), OrdChange::from(value)));
            }
        }
        let expected_set = BTreeSet::from_iter(changes.into_iter().map(|(name, key, value)| {
            (name, key, OrdChange::from(value))
        }));
        assert_eq!(patch_set, expected_set);
    }

    #[test]
    fn test_checkpointdb_basics() {
        let mut db = CheckpointDb::new(MemoryDB::new());
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();
        {
            let journal = db.journal.read().expect(
                "Cannot acquire read lock on journal",
            );
            assert_eq!(journal.len(), 1);
            check_patch(&journal[0], vec![("foo", vec![], Change::Delete)]);
        }

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));

        let mut fork = db.fork();
        fork.put("foo", vec![], vec![3]);
        fork.put("bar", vec![1], vec![4]);
        db.merge(fork.into_patch()).unwrap();
        {
            let journal = db.journal.read().expect(
                "Cannot acquire read lock on journal",
            );
            assert_eq!(journal.len(), 2);
            check_patch(&journal[0], vec![("foo", vec![], Change::Delete)]);
            check_patch(
                &journal[1],
                vec![("foo", vec![], Change::Put(vec![2])), ("bar", vec![1], Change::Delete)],
            );
        }

        // Check that the old snapshot still corresponds to the same DB state
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![3]));
    }

    #[test]
    fn test_checkpointdb_rollback() {
        let mut db = CheckpointDb::new(MemoryDB::new());
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();

        let mut fork = db.fork();
        fork.put("foo", vec![], vec![3]);
        fork.put("bar", vec![1], vec![4]);
        db.merge(fork.into_patch()).unwrap();
        {
            let journal = db.journal.read().expect(
                "Cannot acquire read lock on journal",
            );
            assert_eq!(journal.len(), 2);
        }

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![3]));
        db.rollback(1);
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));
        assert_eq!(snapshot.get("bar", &[1]), None);
        {
            let journal = db.journal.read().expect(
                "Cannot acquire read lock on journal",
            );
            assert_eq!(journal.len(), 1);
        }

        // Check that DB continues working as usual after a rollback
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![4]);
        fork.put("foo", vec![0, 0], vec![255]);
        db.merge(fork.into_patch()).unwrap();
        {
            let journal = db.journal.read().expect(
                "Cannot acquire read lock on journal",
            );
            assert_eq!(journal.len(), 2);
        }
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![4]));
        assert_eq!(snapshot.get("foo", &[0, 0]), Some(vec![255]));

        let mut fork = db.fork();
        fork.put("bar", vec![1], vec![254]);
        db.merge(fork.into_patch()).unwrap();
        {
            let journal = db.journal.read().expect(
                "Cannot acquire read lock on journal",
            );
            assert_eq!(journal.len(), 3);
        }
        let new_snapshot = db.snapshot();
        assert_eq!(new_snapshot.get("foo", &[]), Some(vec![4]));
        assert_eq!(new_snapshot.get("foo", &[0, 0]), Some(vec![255]));
        assert_eq!(new_snapshot.get("bar", &[1]), Some(vec![254]));

        db.rollback(2);
        {
            let journal = db.journal.read().expect(
                "Cannot acquire read lock on journal",
            );
            assert_eq!(journal.len(), 1);
        }
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));
        assert_eq!(snapshot.get("foo", &[0, 0]), None);
        assert_eq!(snapshot.get("bar", &[1]), None);

        assert_eq!(new_snapshot.get("foo", &[]), Some(vec![4]));
        assert_eq!(new_snapshot.get("foo", &[0, 0]), Some(vec![255]));
        assert_eq!(new_snapshot.get("bar", &[1]), Some(vec![254]));
    }

    #[test]
    fn test_checkpointdb_clone() {
        let mut db = CheckpointDb::new(MemoryDB::new());
        let mut db_clone = Database::clone(&db);

        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();

        let snapshot = db_clone.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));

        let mut fork = db_clone.fork();
        fork.put("foo", vec![], vec![3]);
        db_clone.merge(fork.into_patch()).unwrap();

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![3]));

        // Check rollback on clones
        let mut db_clone = Clone::clone(&db);
        db_clone.rollback(1);
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));
        db.rollback(1);
        let snapshot = db_clone.snapshot();
        assert_eq!(snapshot.get("foo", &[]), None);
    }

    #[test]
    fn test_checkpointdb_handler() {
        let mut db = CheckpointDb::new(MemoryDB::new());
        let mut db_handler = db.handler();

        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));

        db_handler.rollback(1);
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), None);
    }
}
