use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use exonum::storage::{Database, Patch, Change, Result as StorageResult, Snapshot};

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

    /// Rolls this database by udoing the latest `count` `merge()` operations.
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
        let mut rev_patch = Patch::new();

        for (name, kv_map) in &patch {
            let mut rev_kv_map = BTreeMap::new();
            for key in kv_map.keys() {
                match snapshot.get(name, key) {
                    Some(value) => {
                        rev_kv_map.insert(key.clone(), Change::Put(value));
                    }
                    None => {
                        rev_kv_map.insert(key.clone(), Change::Delete);
                    }
                }
            }
            rev_patch.insert(name.to_string(), rev_kv_map);
        }

        {
            let mut journal = self.journal.write().expect(
                "Cannot acquire write lock on journal",
            );
            journal.push(rev_patch);
        }
        self.inner.merge(patch)
    }
}

/// Handler to a checkpointed database.
#[derive(Clone, Debug)]
pub struct CheckpointDbHandler<T>(CheckpointDb<T>);

impl<T: Database + Clone> CheckpointDbHandler<T> {
    /// Rolls this database by udoing the latest `count` `merge()` operations.
    pub fn rollback(&mut self, count: usize) -> bool {
        self.0.rollback(count)
    }
}

#[cfg(test)]
mod tests {
    use exonum::storage::MemoryDB;
    use super::*;

    fn create_patch<'a, I>(from: I) -> Patch
    where
        I: IntoIterator<Item = (&'a str, Vec<u8>, Change)>,
    {
        let mut patch = Patch::new();

        for (name, key, value) in from.into_iter() {
            let name = name.to_string();
            let kv_map = patch.entry(name).or_insert_with(BTreeMap::new);
            if kv_map.insert(key, value).is_some() {
                panic!("Attempt to re-insert a key into patch");
            }
        }

        patch
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
            assert_eq!(
                journal[0],
                create_patch(vec![("foo", vec![], Change::Delete)])
            );
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
            assert_eq!(
                journal[0],
                create_patch(vec![("foo", vec![], Change::Delete)])
            );
            assert_eq!(
                journal[1],
                create_patch(vec![
                    ("foo", vec![], Change::Put(vec![2])),
                    ("bar", vec![1], Change::Delete),
                ])
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
