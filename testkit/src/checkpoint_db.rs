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

use std::sync::{Arc, RwLock};
use std::collections::VecDeque;

use exonum::storage::{Database, Patch, Result as StorageResult, Snapshot};

/// Implementation of a `Database`, which allows to rollback commits introduced by the `merge()`
/// function.
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
        CheckpointDb { inner: Arc::new(RwLock::new(CheckpointDbInner::new(db))) }
    }

    /// Returns a handler to the database. The handler could be used to roll the database back
    /// without having the ownership to it.
    pub fn handler(&self) -> CheckpointDbHandler<T> {
        CheckpointDbHandler { inner: Arc::clone(&self.inner) }
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
    /// Set a checkpoint for future rollback
    ///
    /// # Panics
    ///
    /// - Panics if another checkpoint was set before and have not been rolled back to.
    pub fn checkpoint(&self) {
        self.inner
            .write()
            .expect("Cannot lock CheckpointDb for checkpoint")
            .checkpoint();
    }

    /// Rolls back this database by undoing the latest `count` `merge()` operations.
    ///
    /// # Panics
    ///
    /// - Panics if there is no checkpoint to rollback to.
    pub fn rollback(&self) {
        self.inner
            .write()
            .expect("Cannot lock CheckpointDb for rollback")
            .rollback();
    }
}

#[derive(Debug)]
// Journal is used to track patches that need to be applied
// (in the order that they appear) to the database in order
// to restore its state to a checkpoint.
//
// Insertion should occur at the front of the journal,
// while patches should apply sequentially in the order
// in which they are in the VecDeque.
struct CheckpointDbInner<T> {
    db: T,
    journal: VecDeque<Patch>,
    checkpoint_set: bool,
}

impl<T: Database> CheckpointDbInner<T> {
    fn new(db: T) -> Self {
        CheckpointDbInner {
            db,
            journal: VecDeque::new(),
            checkpoint_set: false,
        }
    }

    fn snapshot(&self) -> Box<Snapshot> {
        self.db.snapshot()
    }

    fn merge(&mut self, patch: Patch) -> StorageResult<()> {
        if self.checkpoint_set {
<<<<<<< HEAD
            self.merge_with_logging(patch)
=======
            self.merge_with_journal_logging(patch)
>>>>>>> Fix spelling
        } else {
            self.db.merge(patch)
        }
    }

<<<<<<< HEAD
    fn merge_with_logging(&mut self, patch: Patch) -> StorageResult<()> {
=======
    fn merge_with_journal_logging(&mut self, patch: Patch) -> StorageResult<()> {
>>>>>>> Fix spelling
        // NB: make sure that **both** the db and the journal
        // are updated atomically.
        let snapshot = self.db.snapshot();
        self.db.merge(patch.clone())?;
        let mut rev_fork = self.db.fork();

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

        self.journal.push_front(rev_fork.into_patch());
        Ok(())
    }

    fn checkpoint(&mut self) {
        assert!(
            !self.checkpoint_set,
            "Checkpoint has already been set. There can only be one checkpoint at a time."
        );
        self.checkpoint_set = true;
    }

    fn rollback(&mut self) {
        assert!(self.checkpoint_set, "Checkpoint has not been set yet");
        for patch in self.journal.drain(..) {
            self.db.merge(patch).expect("Cannot merge roll-back patch");
        }
        self.checkpoint_set = false;
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
    fn test_checkpoint_db_basics() {
        let db = CheckpointDb::new(MemoryDB::new());
        let handler = db.handler();

        handler.checkpoint();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let journal = &inner.journal;
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
            let inner = db.inner.read().unwrap();
            let journal = &inner.journal;
            assert_eq!(journal.len(), 2);
            check_patch(&journal[1], vec![("foo", vec![], Change::Delete)]);
            check_patch(
                &journal[0],
                vec![("foo", vec![], Change::Put(vec![2])), ("bar", vec![1], Change::Delete)],
            );
        }

        // Check that the old snapshot still corresponds to the same DB state
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![3]));
    }

    #[test]
    fn test_checkpoint_db_rollback() {
        let db = CheckpointDb::new(MemoryDB::new());
        let handler = db.handler();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();

        handler.checkpoint();

        let mut fork = db.fork();
        fork.put("foo", vec![], vec![3]);
        fork.put("bar", vec![1], vec![4]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let journal = &inner.journal;
            assert_eq!(journal.len(), 1);
        }

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![3]));
        handler.rollback();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));
        assert_eq!(snapshot.get("bar", &[1]), None);
        {
            let inner = db.inner.read().unwrap();
            let journal = &inner.journal;
            assert_eq!(journal.len(), 0);
        }

        // Check that DB continues working as usual after a rollback
        handler.checkpoint();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![4]);
        fork.put("foo", vec![0, 0], vec![255]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let journal = &inner.journal;
            assert_eq!(journal.len(), 1);
        }
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![4]));
        assert_eq!(snapshot.get("foo", &[0, 0]), Some(vec![255]));

        let mut fork = db.fork();
        fork.put("bar", vec![1], vec![254]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let journal = &inner.journal;
            assert_eq!(journal.len(), 2);
        }
        let new_snapshot = db.snapshot();
        assert_eq!(new_snapshot.get("foo", &[]), Some(vec![4]));
        assert_eq!(new_snapshot.get("foo", &[0, 0]), Some(vec![255]));
        assert_eq!(new_snapshot.get("bar", &[1]), Some(vec![254]));

        handler.rollback();
        {
            let inner = db.inner.read().unwrap();
            let journal = &inner.journal;
            assert_eq!(journal.len(), 0);
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
    fn test_checkpoint_db_handler() {
        let db = CheckpointDb::new(MemoryDB::new());
        let db_handler = db.handler();

        db_handler.checkpoint();
        let mut fork = db.fork();
        fork.put("foo", vec![], vec![2]);
        db.merge(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), Some(vec![2]));

        db_handler.rollback();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get("foo", &[]), None);
    }
}
