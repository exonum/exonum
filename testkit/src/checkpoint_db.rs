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

use exonum::storage::{Database, Patch, Result as StorageResult, Snapshot};

use std::sync::{Arc, RwLock};

/// Implementation of a `Database`, which allows to rollback its state
/// to the last made checkpoint.
///
/// **Note:** Intended for testing purposes only. Probably inefficient.
#[derive(Debug)]
pub struct CheckpointDb<T> {
    inner: Arc<RwLock<CheckpointDbInner<T>>>,
}

impl<T: Database> CheckpointDb<T> {
    /// Creates a new checkpointed database that uses the specified `db` as the underlying
    /// data storage.
    pub fn new(db: T) -> Self {
        CheckpointDb {
            inner: Arc::new(RwLock::new(CheckpointDbInner::new(db))),
        }
    }

    /// Returns a handler to the database. The handler could be used to roll the database back
    /// without having the ownership to it.
    pub fn handler(&self) -> CheckpointDbHandler<T> {
        CheckpointDbHandler {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: Database> Database for CheckpointDb<T> {
    fn snapshot(&self) -> Box<Snapshot> {
        self.inner
            .read()
            .expect("Cannot lock CheckpointDb for snapshot")
            .snapshot()
    }

    fn merge(&self, patch: Patch) -> StorageResult<()> {
        self.inner
            .write()
            .expect("Cannot lock CheckpointDb for merge")
            .merge(patch)
    }

    fn merge_sync(&self, patch: Patch) -> StorageResult<()> {
        self.merge(patch)
    }
}

impl<T: Database> From<CheckpointDb<T>> for Arc<Database> {
    fn from(db: CheckpointDb<T>) -> Arc<Database> {
        Arc::from(Box::new(db) as Box<Database>)
    }
}

/// Handler to a checkpointed database, which
/// allows rollback of transactions.
#[derive(Debug)]
pub struct CheckpointDbHandler<T> {
    inner: Arc<RwLock<CheckpointDbInner<T>>>,
}

impl<T: Database> CheckpointDbHandler<T> {
    /// Sets a checkpoint for a future [`rollback`](#method.rollback).
    pub fn checkpoint(&self) {
        self.inner
            .write()
            .expect("Cannot lock checkpointDb for checkpoint")
            .checkpoint();
    }

    /// Rolls back this database to the latest checkpoint
    /// set with [`checkpoint`](#method.checkpoint).
    ///
    /// # Panics
    ///
    /// - Panics if there are no available checkpoints.
    pub fn rollback(&self) {
        self.inner
            .write()
            .expect("Cannot lock CheckpointDb for rollback")
            .rollback();
    }
}

#[derive(Debug)]
struct CheckpointDbInner<T> {
    db: T,
    backup_stack: Vec<Vec<Patch>>,
}

impl<T: Database> CheckpointDbInner<T> {
    fn new(db: T) -> Self {
        CheckpointDbInner {
            db,
            backup_stack: Vec::new(),
        }
    }

    fn snapshot(&self) -> Box<Snapshot> {
        self.db.snapshot()
    }

    fn merge(&mut self, patch: Patch) -> StorageResult<()> {
        if self.backup_stack.is_empty() {
            self.db.merge(patch)
        } else {
            self.merge_with_logging(patch)
        }
    }

    fn merge_with_logging(&mut self, patch: Patch) -> StorageResult<()> {
        // NB: make sure that **both** the db and the journal
        // are updated atomically.
        let snapshot = self.db.snapshot();
        self.db.merge(patch.clone())?;
        let mut rev_fork = self.db.fork();

        // Reverse a patch to get a backup patch.
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

        self.backup_stack
            .last_mut()
            .expect("`merge_with_logging` called before checkpoint has been set")
            .push(rev_fork.into_patch());
        Ok(())
    }

    fn checkpoint(&mut self) {
        self.backup_stack.push(Vec::new())
    }

    fn rollback(&mut self) {
        assert!(
            !self.backup_stack.is_empty(),
            "Checkpoint has not been set yet"
        );
        let changelog = self.backup_stack.pop().unwrap();
        for patch in changelog.into_iter().rev() {
            self.db.merge(patch).expect("Cannot merge roll-back patch");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exonum::storage::{Change, MemoryDB};

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
        let expected_set = BTreeSet::from_iter(
            changes
                .into_iter()
                .map(|(name, key, value)| (name, key, OrdChange::from(value))),
        );
        assert_eq!(patch_set, expected_set);
    }

    fn stack_len<T>(db: &CheckpointDb<T>) -> usize {
        let inner = db.inner.read().unwrap();
        inner.backup_stack.len()
    }

    #[test]
    fn test_backup_stack() {
        let db = CheckpointDb::new(MemoryDB::new());
        let handler = db.handler();

        assert_eq!(stack_len(&db), 0);
        handler.checkpoint();
        assert_eq!(stack_len(&db), 1);
        handler.rollback();
        assert_eq!(stack_len(&db), 0);

        handler.checkpoint();
        handler.checkpoint();
        assert_eq!(stack_len(&db), 2);
        handler.rollback();
        assert_eq!(stack_len(&db), 1);
    }

    #[test]
    fn test_backup() {
        let db = CheckpointDb::new(MemoryDB::new());
        let handler = db.handler();
        handler.checkpoint();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let backup = &inner.backup_stack[0];
            assert_eq!(backup.len(), 1);
            check_patch(&backup[0], vec![("foo", vec![], Change::Delete)]);
        }

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));

        handler.checkpoint();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![3]);
        fork.put("bar", vec![1], vec![4]);
        fork.put("bar2", vec![5], vec![6]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 2);
            let recent_backup = &stack[1];
            let older_backup = &stack[0];
            check_patch(&older_backup[0], vec![("foo", vec![], Change::Delete)]);
            check_patch(
                &recent_backup[0],
                vec![
                    ("bar2", vec![5], Change::Delete),
                    ("bar", vec![1], Change::Delete),
                    ("foo", vec![], Change::Put(vec![2])),
                ],
            );
        }

        // Check that the old snapshot still corresponds to the same DB state.
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![3]));
    }

    #[test]
    fn test_rollback() {
        let db = CheckpointDb::new(MemoryDB::new());
        let handler = db.handler();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();

        // Both checkpoints are on purpose.
        handler.checkpoint();
        handler.checkpoint();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![3]);
        fork.put("bar", vec![1], vec![4]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 2);
            assert_eq!(stack[1].len(), 1);
            assert_eq!(stack[0].len(), 0);
        }

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![3]));
        handler.rollback();

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));
        assert_eq!(snapshot.get("bar", &[1]), None);
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 1);
            assert_eq!(stack[0].len(), 0);
        }

        // Check that DB continues working as usual after a rollback.
        handler.checkpoint();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![4]);
        fork.put("foo", vec![0, 0], vec![255]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 2);
            assert_eq!(stack[1].len(), 1);
            assert_eq!(stack[0].len(), 0);
        }
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![4]));
        assert_eq!(snapshot.get("foo", &[0, 0]), Some(vec![255]));

        let mut fork = db.fork();
        fork.put("bar", vec![1], vec![254]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 2);
            assert_eq!(stack[1].len(), 2);
            assert_eq!(stack[0].len(), 0);
        }
        let new_snapshot = db.snapshot();
        assert_eq!(new_snapshot.get("foo", &[]), Some(vec![4]));
        assert_eq!(new_snapshot.get("foo", &[0, 0]), Some(vec![255]));
        assert_eq!(new_snapshot.get("bar", &[1]), Some(vec![254]));
        handler.rollback();

        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 1);
            assert_eq!(stack[0].len(), 0);
        }
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));
        assert_eq!(snapshot.get("foo", &[0, 0]), None);
        assert_eq!(snapshot.get("bar", &[1]), None);

        assert_eq!(new_snapshot.get("foo", &[]), Some(vec![4]));
        assert_eq!(new_snapshot.get("foo", &[0, 0]), Some(vec![255]));
        assert_eq!(new_snapshot.get("bar", &[1]), Some(vec![254]));
        handler.rollback();

        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 0);
        }
    }

    #[test]
    fn test_handler() {
        let db = CheckpointDb::new(MemoryDB::new());
        let handler = db.handler();

        handler.checkpoint();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));

        handler.rollback();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), None);
    }

    #[test]
    #[should_panic]
    fn test_extra_rollback() {
        let db = CheckpointDb::new(MemoryDB::new());
        let handler = db.handler();

        handler.checkpoint();
        handler.checkpoint();
        handler.rollback();
        handler.rollback();
        handler.rollback();
    }
}
