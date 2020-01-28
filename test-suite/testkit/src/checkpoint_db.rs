// Copyright 2020 The Exonum Team
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

use exonum_merkledb::{Database, DatabaseExt, Patch, Result as StorageResult, Snapshot};

use std::sync::{Arc, RwLock};

/// Implementation of a `Database`, which allows to rollback its state
/// to the last made checkpoint.
///
/// **Note:** Intended for testing purposes only. Probably inefficient.
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
            handle: self.clone(),
        }
    }
}

impl<T> Clone for CheckpointDb<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> std::fmt::Debug for CheckpointDb<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckpointDb")
            .field("refs", &Arc::strong_count(&self.inner))
            .finish()
    }
}

impl<T: Database> Database for CheckpointDb<T> {
    fn snapshot(&self) -> Box<dyn Snapshot> {
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

impl<T: Database> From<CheckpointDb<T>> for Arc<dyn Database> {
    fn from(db: CheckpointDb<T>) -> Arc<dyn Database> {
        Arc::new(db)
    }
}

impl<T: Database> From<T> for CheckpointDb<T> {
    fn from(db: T) -> Self {
        CheckpointDb::new(db)
    }
}

/// Handler to a checkpointed database, which
/// allows rollback of transactions.
#[derive(Debug, Clone)]
pub struct CheckpointDbHandler<T> {
    handle: CheckpointDb<T>,
}

impl<T: Database> CheckpointDbHandler<T> {
    /// Sets a checkpoint for a future [`rollback`](#method.rollback).
    pub fn checkpoint(&self) {
        self.handle
            .inner
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
        self.handle
            .inner
            .write()
            .expect("Cannot lock CheckpointDb for rollback")
            .rollback();
    }

    /// Tries to unwrap this handler.
    pub fn try_unwrap(self) -> Result<T, Self> {
        let lock = Arc::try_unwrap(self.handle.inner).map_err(|inner| {
            eprintln!("strong: {}", Arc::strong_count(&inner));
            Self {
                handle: CheckpointDb { inner },
            }
        })?;
        let inner = lock.into_inner().expect("cannot unwrap `RwLock`");
        Ok(inner.db)
    }

    /// Gets the underlying checkpoint database.
    pub fn into_inner(self) -> CheckpointDb<T> {
        self.handle
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

    fn snapshot(&self) -> Box<dyn Snapshot> {
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
        let backup_patch = self.db.merge_with_backup(patch)?;
        self.backup_stack
            .last_mut()
            .expect("`merge_with_logging` called before checkpoint has been set")
            .push(backup_patch);
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
    use exonum_merkledb::{access::CopyAccessExt, TemporaryDB};

    fn stack_len<T>(db: &CheckpointDb<T>) -> usize {
        let inner = db.inner.read().unwrap();
        inner.backup_stack.len()
    }

    #[test]
    fn backup_stack_length() {
        let db = CheckpointDb::new(TemporaryDB::new());
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
    #[allow(clippy::cognitive_complexity)]
    fn interleaved_rollbacks() {
        let db = CheckpointDb::new(TemporaryDB::new());
        let handler = db.handler();
        let fork = db.fork();
        fork.get_list("foo").push(1_u32);
        fork.get_list("bar").push("...".to_owned());
        db.merge(fork.into_patch()).unwrap();

        // Both checkpoints are on purpose.
        handler.checkpoint();
        handler.checkpoint();
        let fork = db.fork();
        fork.get_list("foo").push(2_u32);
        fork.get_list("bar").set(0, "!".to_owned());
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 2);
            assert_eq!(stack[1].len(), 1);
            assert_eq!(stack[0].len(), 0);
        }

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get_list::<_, u32>("foo").len(), 2);
        assert_eq!(
            snapshot.get_list("foo").iter().collect::<Vec<u32>>(),
            vec![1, 2]
        );
        assert_eq!(snapshot.get_list::<_, String>("bar").len(), 1);
        assert_eq!(
            snapshot.get_list::<_, String>("bar").get(0),
            Some("!".to_owned())
        );
        handler.rollback();

        let snapshot = db.snapshot();
        assert_eq!(snapshot.get_list::<_, u32>("foo").len(), 1);
        assert_eq!(
            snapshot.get_list("foo").iter().collect::<Vec<u32>>(),
            vec![1]
        );
        assert_eq!(snapshot.get_list::<_, String>("bar").len(), 1);
        assert_eq!(
            snapshot.get_list::<_, String>("bar").get(0),
            Some("...".to_owned())
        );

        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 1);
            assert_eq!(stack[0].len(), 0);
        }

        // Check that DB continues working as usual after a rollback.
        handler.checkpoint();
        let fork = db.fork();
        fork.get_list("foo").push(3_u32);
        fork.get_list("bar")
            .extend(vec!["?".to_owned(), ".".to_owned()]);
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 2);
            assert_eq!(stack[1].len(), 1);
            assert_eq!(stack[0].len(), 0);
        }
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get_list::<_, u32>("foo").len(), 2);
        assert_eq!(snapshot.get_list::<_, u32>("bar").len(), 3);

        let fork = db.fork();
        fork.get_list("foo").push(4_u32);
        fork.get_list::<_, String>("bar").clear();
        db.merge(fork.into_patch()).unwrap();
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 2);
            assert_eq!(stack[1].len(), 2);
            assert_eq!(stack[0].len(), 0);
        }
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get_list::<_, u32>("foo").len(), 3);
        assert_eq!(
            snapshot.get_list("foo").iter().collect::<Vec<u32>>(),
            vec![1, 3, 4]
        );
        assert!(snapshot.get_list::<_, String>("bar").is_empty());

        handler.rollback();
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 1);
            assert_eq!(stack[0].len(), 0);
        }
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get_list::<_, u32>("foo").len(), 1);
        assert_eq!(
            snapshot.get_list("foo").iter().collect::<Vec<u32>>(),
            vec![1]
        );
        assert_eq!(snapshot.get_list::<_, String>("bar").len(), 1);
        assert_eq!(
            snapshot.get_list("bar").iter().collect::<Vec<String>>(),
            vec!["...".to_owned()]
        );

        handler.rollback();
        {
            let inner = db.inner.read().unwrap();
            let stack = &inner.backup_stack;
            assert_eq!(stack.len(), 0);
        }
    }

    #[test]
    fn rollback_via_handler() {
        let db = CheckpointDb::new(TemporaryDB::new());
        let handler = db.handler();

        handler.checkpoint();
        let fork = db.fork();
        fork.get_entry("foo").set(42_u32);
        db.merge(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(snapshot.get_entry::<_, u32>("foo").get(), Some(42));

        handler.rollback();
        let snapshot = db.snapshot();
        assert!(!snapshot.get_entry::<_, u32>("foo").exists());
    }

    #[test]
    #[should_panic(expected = "Checkpoint has not been set yet")]
    fn extra_rollback() {
        let db = CheckpointDb::new(TemporaryDB::new());
        let handler = db.handler();

        handler.checkpoint();
        handler.checkpoint();
        handler.rollback();
        handler.rollback();
        handler.rollback();
    }
}
