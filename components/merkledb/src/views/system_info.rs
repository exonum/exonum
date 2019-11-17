use exonum_crypto::Hash;

use super::{metadata::IndexesPool, AsReadonly, RawAccess};
use crate::{access::AccessExt, Fork, ObjectHash, ProofMapIndex};

/// Name of the state aggregator proof map.
pub(super) const STATE_AGGREGATOR: &str = "__STATE_AGGREGATOR__";

/// System-wide information about the database.
#[derive(Debug, Clone, Copy)]
pub struct SystemInfo<T>(T);

impl<T: RawAccess> SystemInfo<T> {
    /// Creates an instance based on the specified `access`.
    pub fn new(access: T) -> Self {
        SystemInfo(access)
    }

    /// Returns the total number of indexes in the storage.
    pub fn index_count(&self) -> u64 {
        IndexesPool::new(self.0.clone()).len() - 1
    }

    /// Returns the state hash of the database.
    pub fn state_hash(&self) -> Hash {
        self.0
            .clone()
            .get_proof_map::<_, String, Hash>(STATE_AGGREGATOR)
            .object_hash()
    }
}

impl<T: RawAccess + AsReadonly> SystemInfo<T> {
    /// Returns the state aggregator of the database.
    pub fn state_aggregator(&self) -> ProofMapIndex<T::Readonly, String, Hash> {
        self.0.as_readonly().get_proof_map(STATE_AGGREGATOR)
    }
}

impl SystemInfo<&Fork> {
    /// Updates state hash of the database.
    pub(crate) fn update_state_aggregator(
        &mut self,
        entries: impl IntoIterator<Item = (String, Hash)>,
    ) {
        let mut state_aggregator = self.0.get_proof_map(STATE_AGGREGATOR);
        for (index_name, hash) in entries {
            state_aggregator.put(&index_name, hash);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, TemporaryDB};

    #[test]
    fn index_count_is_correct() {
        let db = TemporaryDB::new();
        let snapshot = db.snapshot();
        assert_eq!(SystemInfo::new(&snapshot).index_count(), 0);

        let fork = db.fork();
        fork.get_list("list").push(1_u32);
        assert_eq!(SystemInfo::new(&fork).index_count(), 1);
        fork.get_map(("map", &0_u8)).put(&1_u32, "!".to_owned());
        let info = SystemInfo::new(&fork);
        assert_eq!(info.index_count(), 2);
        fork.get_map(("map", &1_u8)).put(&1_u32, "!".to_owned());
        assert_eq!(info.index_count(), 3);

        fork.get_map(("map", &0_u8)).put(&2_u32, "!".to_owned());
        assert_eq!(SystemInfo::new(&fork).index_count(), 3);
        fork.get_list("list").push(5_u32);
        assert_eq!(SystemInfo::new(&fork).index_count(), 3);

        db.merge_sync(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        assert_eq!(SystemInfo::new(&snapshot).index_count(), 3);

        let fork = db.fork();
        fork.get_list("list").push(1_u32);
        assert_eq!(SystemInfo::new(&fork).index_count(), 3);
        fork.get_list("other_list").push(1_u32);
        assert_eq!(SystemInfo::new(&fork).index_count(), 4);
        assert_eq!(SystemInfo::new(fork.readonly()).index_count(), 4);

        assert_eq!(SystemInfo::new(&snapshot).index_count(), 3);
    }

    fn initial_changes(fork: &Fork) {
        fork.get_proof_list("list").extend(vec![1_u32, 2, 3]);
        fork.get_list("non_hashed_list").push(1_u64);
        {
            let mut map = fork.get_proof_map("map");
            for i in 0..5 {
                map.put(&i, i.to_string());
            }
        }
        fork.get_proof_list(("grouped_list", &1_u8)).push(5_u8);
    }

    fn further_changes(fork: &Fork) {
        fork.get_proof_list::<_, u32>("list").clear();
        fork.get_map("non_hashed_map").put(&1_u32, "!".to_owned());
        fork.get_proof_list("list").push(1_u32);
        fork.get_proof_map("another_map")
            .put(&1_u64, "?".to_owned());
    }

    #[test]
    fn state_update() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        initial_changes(&fork);

        let patch = fork.into_patch();
        let info = SystemInfo::new(&patch);
        let aggregator = info.state_aggregator();
        assert_eq!(
            aggregator.keys().collect::<Vec<_>>(),
            vec!["list".to_owned(), "map".to_owned()]
        );
        assert_eq!(
            aggregator.get(&"list".to_owned()).unwrap(),
            patch.get_proof_list::<_, u32>("list").object_hash()
        );
        assert_eq!(
            aggregator.get(&"map".to_owned()).unwrap(),
            patch.get_proof_map::<_, i32, String>("map").object_hash()
        );
        assert_eq!(aggregator.object_hash(), info.state_hash());
    }

    #[test]
    fn state_update_after_merge() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        initial_changes(&fork);
        db.merge_sync(fork.into_patch()).unwrap();
        let fork = db.fork();
        further_changes(&fork);

        let patch = fork.into_patch();
        let info = SystemInfo::new(&patch);
        let aggregator = info.state_aggregator();
        assert_eq!(
            aggregator.keys().collect::<Vec<_>>(),
            vec![
                "another_map".to_owned(),
                "list".to_owned(),
                "map".to_owned(),
            ]
        );
        assert_eq!(
            aggregator.get(&"list".to_owned()).unwrap(),
            patch.get_proof_list::<_, u32>("list").object_hash()
        );
        assert_eq!(
            aggregator.get(&"map".to_owned()).unwrap(),
            patch.get_proof_map::<_, i32, String>("map").object_hash()
        );
        assert_eq!(aggregator.object_hash(), info.state_hash());
        db.merge_sync(patch).unwrap();

        let snapshot = db.snapshot();
        let info = SystemInfo::new(&snapshot);
        let aggregator = info.state_aggregator();
        assert_eq!(
            aggregator.keys().collect::<Vec<_>>(),
            vec![
                "another_map".to_owned(),
                "list".to_owned(),
                "map".to_owned(),
            ]
        );
        assert_eq!(
            aggregator.get(&"list".to_owned()).unwrap(),
            snapshot.get_proof_list::<_, u32>("list").object_hash()
        );
        assert_eq!(
            aggregator.get(&"map".to_owned()).unwrap(),
            snapshot
                .get_proof_map::<_, i32, String>("map")
                .object_hash()
        );
        assert_eq!(aggregator.object_hash(), info.state_hash());
    }
}
