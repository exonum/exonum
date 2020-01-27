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

//! Checks that state aggregator is updated each time `Fork` is converted into a `Patch`.

use proptest::{
    collection::vec, num, prop_assert, prop_assert_eq, prop_oneof, proptest, strategy,
    strategy::Strategy, test_runner::TestCaseResult,
};

use std::{collections::HashSet, iter::FromIterator};

use exonum_merkledb::{
    access::CopyAccessExt, Database, IndexAddress, ObjectHash, Patch, SystemSchema, TemporaryDB,
};

const ACTIONS_MAX_LEN: usize = 50;

#[derive(Debug, Clone)]
enum Action {
    PushToList {
        addr: IndexAddress,
        value: u64,
    },
    ClearList {
        addr: IndexAddress,
    },
    PushToMap {
        addr: IndexAddress,
        key: u64,
        value: u64,
    },
    ClearMap {
        addr: IndexAddress,
    },
    SetEntry {
        addr: IndexAddress,
        value: u64,
    },
    UnsetEntry {
        addr: IndexAddress,
    },
    Flush,
    IntoPatch,
    Merge,
}

fn list_addrs() -> impl Strategy<Value = IndexAddress> {
    prop_oneof![
        strategy::Just("list".into()),
        strategy::Just("other_list".into()),
        strategy::Just("another_list".into()),
        strategy::Just("prefixed.list".into()),
        strategy::Just(("group", &1_u8).into()),
    ]
}

fn map_addrs() -> impl Strategy<Value = IndexAddress> {
    prop_oneof![
        strategy::Just("map".into()),
        strategy::Just("other_map".into()),
        strategy::Just("another_map".into()),
        strategy::Just("prefixed.map".into()),
        strategy::Just(("group", &2_u8).into()),
    ]
}

fn entry_addrs() -> impl Strategy<Value = IndexAddress> {
    prop_oneof![
        strategy::Just("entry".into()),
        strategy::Just("other_entry".into()),
        strategy::Just(("group", &3_u8).into()),
    ]
}

fn generate_action() -> impl Strategy<Value = Action> {
    prop_oneof![
        (list_addrs(), num::u64::ANY).prop_map(|(addr, value)| Action::PushToList { addr, value }),
        list_addrs().prop_map(|addr| Action::ClearList { addr }),
        (map_addrs(), num::u64::ANY, num::u64::ANY)
            .prop_map(|(addr, key, value)| Action::PushToMap { addr, key, value }),
        map_addrs().prop_map(|addr| Action::ClearMap { addr }),
        (entry_addrs(), num::u64::ANY).prop_map(|(addr, value)| Action::SetEntry { addr, value }),
        entry_addrs().prop_map(|addr| Action::UnsetEntry { addr }),
        strategy::Just(Action::Flush),
        strategy::Just(Action::IntoPatch),
        strategy::Just(Action::Merge),
    ]
}

fn check_patch(patch: &Patch) -> TestCaseResult {
    const LIST_NAMES: &[&str] = &["list", "other_list", "another_list", "prefixed.list"];
    const MAP_NAMES: &[&str] = &["map", "other_map", "another_map", "prefixed.map"];
    const ENTRY_NAMES: &[&str] = &["entry", "other_entry"];

    let system_schema = SystemSchema::new(patch);
    let state_hash = system_schema.state_hash();
    let aggregator = system_schema.state_aggregator();
    prop_assert_eq!(state_hash, aggregator.object_hash());

    let mut absent_lists: HashSet<_> =
        HashSet::from_iter(LIST_NAMES.iter().map(|&s| s.to_string()));
    let mut absent_maps: HashSet<_> = HashSet::from_iter(MAP_NAMES.iter().map(|&s| s.to_string()));
    let mut absent_entries: HashSet<_> =
        HashSet::from_iter(ENTRY_NAMES.iter().map(|&s| s.to_string()));

    for (index_name, index_hash) in aggregator.iter() {
        let is_list = LIST_NAMES.contains(&index_name.as_str());
        let is_map = MAP_NAMES.contains(&index_name.as_str());
        let is_entry = ENTRY_NAMES.contains(&index_name.as_str());
        prop_assert!(is_list || is_map || is_entry);

        if is_list {
            let list = patch.get_proof_list::<_, u64>(index_name.as_str());
            prop_assert_eq!(list.object_hash(), index_hash);
            absent_lists.remove(&index_name);
        } else if is_map {
            let map = patch.get_proof_map::<_, u64, u64>(index_name.as_str());
            prop_assert_eq!(map.object_hash(), index_hash);
            absent_maps.remove(&index_name);
        } else {
            let entry = patch.get_proof_entry::<_, u64>(index_name.as_str());
            prop_assert_eq!(entry.object_hash(), index_hash);
            absent_entries.remove(&index_name);
        }

        let proof = aggregator.get_proof(index_name);
        let proof = proof.check_against_hash(state_hash).unwrap();
        prop_assert_eq!(proof.entries().count(), 1);
    }

    for index_name in absent_lists {
        let list = patch.get_proof_list::<_, u64>(index_name.as_str());
        prop_assert!(list.is_empty());
    }
    for index_name in absent_maps {
        let map = patch.get_proof_map::<_, u64, u64>(index_name.as_str());
        prop_assert_eq!(map.iter().count(), 0);
    }
    for index_name in absent_entries {
        let entry = patch.get_entry::<_, u64>(index_name.as_str());
        prop_assert!(!entry.exists());
    }

    Ok(())
}

fn test_aggregator(db: &TemporaryDB, actions: Vec<Action>) -> TestCaseResult {
    let mut fork = db.fork();
    for action in actions {
        match action {
            Action::PushToList { addr, value } => {
                let mut list = fork.get_proof_list::<_, u64>(addr);
                list.push(value);
            }
            Action::ClearList { addr } => {
                let mut list = fork.get_proof_list::<_, u64>(addr);
                list.clear();
            }

            Action::PushToMap { addr, key, value } => {
                let mut map = fork.get_proof_map::<_, u64, u64>(addr);
                map.put(&key, value);
            }
            Action::ClearMap { addr } => {
                let mut map = fork.get_proof_map::<_, u64, u64>(addr);
                map.clear();
            }

            Action::SetEntry { addr, value } => {
                let mut entry = fork.get_proof_entry::<_, u64>(addr);
                entry.set(value);
            }
            Action::UnsetEntry { addr } => {
                let mut entry = fork.get_proof_entry::<_, u64>(addr);
                entry.take();
            }

            Action::Flush => {
                fork.flush();
            }
            Action::IntoPatch | Action::Merge => {
                let patch = fork.into_patch();
                check_patch(&patch)?;
                fork = if let Action::Merge = action {
                    db.merge(patch).unwrap();
                    db.fork()
                } else {
                    patch.into()
                };
            }
        }
    }
    Ok(())
}

#[test]
fn aggregator_is_kept_up_to_date() {
    let db = TemporaryDB::new();
    proptest!(|(actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
        test_aggregator(&db, actions)?;
    });
}
