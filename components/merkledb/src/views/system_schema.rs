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

use exonum_crypto::Hash;

use super::{AsReadonly, IndexType, RawAccess, ViewWithMetadata};
use crate::{Fork, ObjectHash, ProofMapIndex};

/// Name of the state aggregator proof map.
const STATE_AGGREGATOR: &str = "__STATE_AGGREGATOR__";

pub fn get_state_aggregator<T: RawAccess>(
    access: T,
    namespace: &str,
) -> ProofMapIndex<T, str, Hash> {
    let view = ViewWithMetadata::get_or_create_unchecked(
        access,
        &(STATE_AGGREGATOR, namespace).into(),
        IndexType::ProofMap,
    )
    .expect("Internal MerkleDB failure while aggregating state");
    ProofMapIndex::new(view)
}

/// System-wide information about the database.
///
/// # Examples
///
/// ```
/// # use exonum_merkledb::{access::CopyAccessExt, Database, ObjectHash, TemporaryDB, SystemSchema};
/// let db = TemporaryDB::new();
/// let fork = db.fork();
/// fork.get_proof_list("list").extend(vec![1_u32, 2, 3]);
/// fork.get_map(("plain_map", &1)).put(&1_u8, "so plain".to_owned());
/// fork.get_map(("plain_map", &2)).put(&2_u8, "s0 plane".to_owned());
///
/// let patch = fork.into_patch();
/// let state_hash = SystemSchema::new(&patch).state_hash();
/// // ^-- State hash of the entire database including changes in the `patch`.
/// db.merge(patch).unwrap();
///
/// let snapshot = db.snapshot();
/// let aggregator = SystemSchema::new(&snapshot).state_aggregator();
/// assert_eq!(aggregator.object_hash(), state_hash);
/// assert_eq!(aggregator.keys().collect::<Vec<_>>(), vec!["list".to_owned()]);
/// // ^-- No other aggregated indexes so far.
/// let index_hash = aggregator.get(&"list".to_owned()).unwrap();
/// assert_eq!(
///     index_hash,
///     snapshot.get_proof_list::<_, u32>("list").object_hash()
/// );
///
/// // It is possible to prove that an index has a specific state
/// // given `state_hash`:
/// let proof = aggregator.get_proof("list".to_owned());
/// proof.check_against_hash(state_hash).unwrap();
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SystemSchema<T>(T);

impl<T: RawAccess> SystemSchema<T> {
    /// Creates an instance based on the specified `access`.
    pub fn new(access: T) -> Self {
        SystemSchema(access)
    }

    /// Returns the state hash of the database. The state hash is up to date for `Snapshot`s
    /// (including `Patch`es), but is generally stale for `Fork`s.
    ///
    /// See [state aggregation] for details how the database state is aggregated.
    ///
    /// [state aggregation]: index.html#state-aggregation
    pub fn state_hash(&self) -> Hash {
        get_state_aggregator(self.0.clone(), "").object_hash()
    }
}

impl<T: RawAccess + AsReadonly> SystemSchema<T> {
    /// Returns the state aggregator of the database. The aggregator is up to date for `Snapshot`s
    /// (including `Patch`es), but is generally stale for `Fork`s.
    ///
    /// See [state aggregation] for details how the database state is aggregated.
    ///
    /// [state aggregation]: index.html#state-aggregation
    pub fn state_aggregator(&self) -> ProofMapIndex<T::Readonly, str, Hash> {
        get_state_aggregator(self.0.as_readonly(), "")
    }
}

impl SystemSchema<&Fork> {
    /// Updates state hash of the database.
    pub(crate) fn update_state_aggregators(
        &mut self,
        entries: impl IntoIterator<Item = (String, String, Hash)>,
    ) {
        for (ns, index_name, hash) in entries {
            get_state_aggregator(self.0, &ns).put(&index_name, hash);
        }
    }

    /// Removes indexes with the specified names from the aggregated indexes
    /// in the default namespace.
    pub(crate) fn remove_aggregated_indexes(&mut self, names: impl IntoIterator<Item = String>) {
        let mut aggregator = get_state_aggregator(self.0, "");
        for name in names {
            aggregator.remove(&name);
        }
    }

    /// Removes an aggregation namespace, moving all aggregated indexes in the namespace into
    /// the default aggregator.
    pub(crate) fn merge_namespace(&mut self, namespace: &str) {
        debug_assert!(!namespace.is_empty(), "Cannot remove default namespace");

        let mut ns_aggregator = get_state_aggregator(self.0, namespace);
        let mut default_aggregator = get_state_aggregator(self.0, "");
        for (index_name, hash) in &ns_aggregator {
            default_aggregator.put(&index_name, hash);
        }
        ns_aggregator.clear();
    }

    /// Removes an aggregation namespace.
    pub(crate) fn remove_namespace(&mut self, namespace: &str) {
        get_state_aggregator(self.0, namespace).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        access::{AccessExt, CopyAccessExt},
        migration::Migration,
        Database, HashTag, TemporaryDB,
    };

    fn initial_changes(fork: &Fork) {
        fork.get_proof_list("list").extend(vec![1_u32, 2, 3]);
        fork.get_list("non_hashed_list").push(1_u64);
        fork.get_proof_entry("entry").set("oops!".to_owned());
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
        let system_schema = SystemSchema::new(&patch);
        let aggregator = system_schema.state_aggregator();
        assert_eq!(
            aggregator.keys().collect::<Vec<_>>(),
            vec!["entry".to_owned(), "list".to_owned(), "map".to_owned()]
        );
        assert_eq!(
            aggregator.get(&"entry".to_owned()).unwrap(),
            patch.get_proof_entry::<_, String>("entry").object_hash()
        );
        assert_eq!(
            aggregator.get(&"list".to_owned()).unwrap(),
            patch.get_proof_list::<_, u32>("list").object_hash()
        );
        assert_eq!(
            aggregator.get(&"map".to_owned()).unwrap(),
            patch.get_proof_map::<_, i32, String>("map").object_hash()
        );
        assert_eq!(aggregator.object_hash(), system_schema.state_hash());
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
        let system_schema = SystemSchema::new(&patch);
        let aggregator = system_schema.state_aggregator();
        let expected_index_names = vec![
            "another_map".to_owned(),
            "entry".to_owned(),
            "list".to_owned(),
            "map".to_owned(),
        ];
        assert_eq!(aggregator.keys().collect::<Vec<_>>(), expected_index_names);
        assert_eq!(
            aggregator.get(&"list".to_owned()).unwrap(),
            patch.get_proof_list::<_, u32>("list").object_hash()
        );
        assert_eq!(
            aggregator.get(&"map".to_owned()).unwrap(),
            patch.get_proof_map::<_, i32, String>("map").object_hash()
        );
        assert_eq!(aggregator.object_hash(), system_schema.state_hash());
        db.merge_sync(patch).unwrap();

        let snapshot = db.snapshot();
        let system_schema = SystemSchema::new(&snapshot);
        let aggregator = system_schema.state_aggregator();
        assert_eq!(aggregator.keys().collect::<Vec<_>>(), expected_index_names);
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
        assert_eq!(aggregator.object_hash(), system_schema.state_hash());
    }

    #[test]
    fn migrated_indexes_do_not_influence_state_hash() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut map = fork.get_map("test.map");
            map.put(&1_u64, "foo".to_owned());
            map.put(&2_u64, "bar".to_owned());
        }
        db.merge(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        let system_schema = SystemSchema::new(&snapshot);
        assert_eq!(system_schema.state_hash(), HashTag::empty_map_hash());

        // Create a merkelized index in a migration. It should not be aggregated.
        let fork = db.fork();
        {
            let migration = Migration::new("test", &fork);
            let mut map = migration.get_proof_map("map");
            map.put(&1_u64, "1".to_owned());
            map.put(&3_u64, "3".to_owned());

            let map = fork.get_map::<_, u64, String>("test.map");
            assert_eq!(map.get(&1).unwrap(), "foo");
            assert!(map.get(&3).is_none());
        }
        db.merge(fork.into_patch()).unwrap();
        let snapshot = db.snapshot();
        let system_schema = SystemSchema::new(&snapshot);
        assert_eq!(system_schema.state_hash(), HashTag::empty_map_hash());
    }
}
