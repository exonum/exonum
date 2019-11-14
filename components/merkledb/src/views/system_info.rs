use exonum_crypto::Hash;

use super::{metadata::AggregatedIndexes, metadata::IndexesPool, AsReadonly, RawAccess};
use crate::{access::AccessExt, Fork, ObjectHash, ProofMapIndex};

/// Name of the state aggregator proof map.
pub(super) const STATE_AGGREGATOR: &str = "__STATE_AGGREGATOR__";

#[derive(Debug, Clone, Copy)]
pub struct SystemInfo<T>(T);

impl<T: RawAccess> SystemInfo<T> {
    pub fn new(access: T) -> Self {
        SystemInfo(access)
    }

    /// Returns the total number of indexes in the storage.
    pub fn index_count(&self) -> u64 {
        IndexesPool::new(self.0.clone()).len()
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

impl<'a> SystemInfo<&'a mut Fork> {
    pub fn mutable(fork: &'a mut Fork) -> Self {
        SystemInfo(fork)
    }

    /// Updates state hash of the database.
    pub fn update_state_aggregator(&mut self) -> Hash {
        // Because the method takes `&mut Fork`, it is guaranteed that no indexes
        // are borrowed at this point, thus, no errors can occur in `AggregatedIndexes::iter()`.
        let state_hash = {
            let mut state_aggregator = self.0.get_proof_map(STATE_AGGREGATOR);
            for (index_name, hash) in AggregatedIndexes::new(&*self.0).iter() {
                state_aggregator.put(&index_name, hash);
            }
            state_aggregator.object_hash()
        };
        self.0.flush();
        state_hash
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

    #[test]
    fn state_update() {
        let db = TemporaryDB::new();
        let mut fork = db.fork();

        fork.get_proof_list("list").extend(vec![1_u32, 2, 3]);
        fork.get_list("non_hashed_list").push(1_u64);
        {
            // Note that the test won't compile without dropping the `map` before constructing
            // `SystemInfo::mutable`.
            let mut map = fork.get_proof_map("map");
            for i in 0..5 {
                map.put(&i, i.to_string());
            }
        }
        fork.get_proof_list(("grouped_list", &1_u8)).push(5_u8);

        let hash = SystemInfo::mutable(&mut fork).update_state_aggregator();
        let info = SystemInfo::new(&fork);
        assert_eq!(hash, info.state_hash());
        let aggregator = info.state_aggregator();
        assert_eq!(
            aggregator.keys().collect::<Vec<_>>(),
            vec!["list".to_owned(), "map".to_owned()]
        );
        assert_eq!(
            aggregator.get(&"list".to_owned()).unwrap(),
            fork.get_proof_list::<_, u32>("list").object_hash()
        );
        assert_eq!(
            aggregator.get(&"map".to_owned()).unwrap(),
            fork.get_proof_map::<_, i32, String>("map").object_hash()
        );
    }
}
