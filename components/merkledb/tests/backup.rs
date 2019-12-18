// Copyright 2019 The Exonum Team
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

//! Tests `DatabaseExt::merge_with_backup()`.

use proptest::{
    collection::vec,
    option, prop_assert, prop_assert_eq, prop_oneof, proptest, sample, strategy,
    strategy::Strategy,
    test_runner::{Config, TestCaseResult},
};

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    iter::FromIterator,
};

use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    Database, DatabaseExt, IndexAddress, IndexType, SystemSchema, TemporaryDB,
};

const INDEX_NAMES: &[&str] = &[
    "foo",
    "bar",
    "b",
    "overly_long_prefix_still_should_work_though",
];

const ACTIONS_MAX_LEN: usize = 20;

#[derive(Debug, Clone)]
enum Action {
    WorkOnIndex {
        addr: IndexAddress,
        index_type: IndexType,
        value: Option<Vec<u8>>,
    },
    FlushFork,
}

fn work_on_index<T>(
    fork: T,
    addr: IndexAddress,
    mut index_type: IndexType,
    value: Option<Vec<u8>>,
) -> IndexType
where
    T: Access + Copy,
    T::Base: RawAccessMut,
{
    if let Some(real_type) = fork.index_type(addr.clone()) {
        index_type = real_type;
    }

    match index_type {
        IndexType::Entry => {
            let mut entry = fork.get_entry(addr);
            if let Some(val) = value {
                entry.set(val);
            } else {
                entry.remove();
            }
        }
        IndexType::ProofEntry => {
            let mut entry = fork.get_proof_entry(addr);
            if let Some(val) = value {
                entry.set(val);
            } else {
                entry.remove();
            }
        }

        IndexType::List => {
            let mut list = fork.get_list(addr);
            if let Some(val) = value {
                list.push(val);
            } else {
                list.clear();
            }
        }
        IndexType::ProofList => {
            let mut list = fork.get_proof_list(addr);
            if let Some(val) = value {
                list.push(val);
            } else {
                list.clear();
            }
        }

        IndexType::Map => {
            let mut map = fork.get_map(addr);
            if let Some(val) = value {
                let key = val[0];
                map.put(&key, val);
            } else {
                map.clear();
            }
        }
        IndexType::ProofMap => {
            let mut map = fork.get_proof_map(addr);
            if let Some(val) = value {
                let key = val[0];
                map.put(&key, val);
            } else {
                map.clear();
            }
        }

        _ => {}
    }

    index_type
}

/// Generates an `IndexAddress` optionally placed in a group.
fn generate_address() -> impl Strategy<Value = IndexAddress> {
    let index_name = sample::select(INDEX_NAMES).prop_map(IndexAddress::from_root);
    prop_oneof![
        // Non-prefixed addresses
        index_name.clone(),
        // Prefixed addresses
        (index_name, 1_u8..8).prop_map(|(addr, prefix)| addr.append_key(&prefix)),
    ]
}

fn generate_index_type() -> impl Strategy<Value = IndexType> {
    prop_oneof![
        strategy::Just(IndexType::Entry),
        strategy::Just(IndexType::ProofEntry),
        strategy::Just(IndexType::List),
        strategy::Just(IndexType::ProofList),
        strategy::Just(IndexType::Map),
        strategy::Just(IndexType::ProofMap),
    ]
}

/// Generates a value to place in the index. if `None` is generated, the index will be cleared
/// instead.
fn generate_value() -> impl Strategy<Value = Option<Vec<u8>>> {
    option::weighted(0.8, vec(0_u8..4, 1..=1))
}

fn generate_action() -> impl Strategy<Value = Action> + Clone {
    let work = (generate_address(), generate_index_type(), generate_value()).prop_map(
        |(addr, index_type, value)| Action::WorkOnIndex {
            addr,
            index_type,
            value,
        },
    );
    prop_oneof![
        4 => work,
        1 => strategy::Just(Action::FlushFork),
    ]
}

#[derive(Debug, Clone)]
struct IndexData {
    ty: IndexType,
    values: Vec<Vec<u8>>,
}

impl IndexData {
    fn check<S>(&self, snapshot: S, addr: IndexAddress) -> TestCaseResult
    where
        S: Access,
    {
        match self.ty {
            IndexType::Entry => {
                let val = snapshot.get_entry::<_, Vec<u8>>(addr).get();
                prop_assert_eq!(val.as_ref(), self.values.last());
            }
            IndexType::ProofEntry => {
                let val = snapshot.get_proof_entry::<_, Vec<u8>>(addr).get();
                prop_assert_eq!(val.as_ref(), self.values.last());
            }

            IndexType::List => {
                let list = snapshot.get_list::<_, Vec<u8>>(addr);
                prop_assert_eq!(list.len(), self.values.len() as u64);
                let values = list.iter().collect::<Vec<_>>();
                prop_assert_eq!(&values, &self.values);
            }
            IndexType::ProofList => {
                let list = snapshot.get_proof_list::<_, Vec<u8>>(addr);
                prop_assert_eq!(list.len(), self.values.len() as u64);
                let values = list.iter().collect::<Vec<_>>();
                prop_assert_eq!(&values, &self.values);
            }

            IndexType::Map => {
                let map = snapshot.get_map::<_, u8, Vec<u8>>(addr);
                let expected_map =
                    BTreeMap::from_iter(self.values.iter().map(|val| (val[0], val.clone())));
                // Using `Vec<_>` allows to test for duplicate entries during iteration etc.
                let expected_map: Vec<_> = expected_map.into_iter().collect();
                prop_assert_eq!(map.iter().collect::<Vec<_>>(), expected_map);
            }
            IndexType::ProofMap => {
                let map = snapshot.get_proof_map::<_, u8, Vec<u8>>(addr);
                let expected_map =
                    BTreeMap::from_iter(self.values.iter().map(|val| (val[0], val.clone())));
                let expected_map: Vec<_> = expected_map.into_iter().collect();
                prop_assert_eq!(map.iter().collect::<Vec<_>>(), expected_map);
            }

            _ => {}
        }
        Ok(())
    }
}

fn check_index_does_not_exist<S: Access>(snapshot: S, addr: IndexAddress) -> TestCaseResult {
    let maybe_type = snapshot.index_type(addr);
    prop_assert!(maybe_type.is_none(), "{:?}", maybe_type.unwrap());
    Ok(())
}

fn apply_actions(
    db: &TemporaryDB,
    initial_actions: Vec<Action>,
    main_actions: Vec<Action>,
) -> TestCaseResult {
    let fork = db.fork();
    let mut initial_data = HashMap::new();

    for action in initial_actions {
        if let Action::WorkOnIndex {
            addr,
            index_type,
            value,
        } = action
        {
            let real_type = work_on_index(&fork, addr.clone(), index_type, value.clone());
            let entry = initial_data.entry(addr).or_insert_with(|| IndexData {
                ty: real_type,
                values: vec![],
            });

            if let Some(value) = value {
                entry.values.push(value);
            } else {
                entry.values.clear();
            }
        }
    }

    // This is the initial database state.
    let patch = fork.into_patch();
    // Check the index contents just in case.
    for (addr, data) in &initial_data {
        data.check(&patch, addr.to_owned())?;
    }

    let system_schema = SystemSchema::new(&patch);
    let initial_state_hash = system_schema.state_hash();
    let initial_state_entries: HashMap<_, _> = system_schema.state_aggregator().iter().collect();
    db.merge(patch).unwrap();

    // Perform main actions.
    let mut fork = db.fork();
    let mut index_data = initial_data.clone();
    let mut new_indexes = HashSet::new();
    for action in main_actions {
        match action {
            Action::WorkOnIndex {
                addr,
                index_type,
                value,
            } => {
                let real_type = work_on_index(&fork, addr.clone(), index_type, value.clone());
                let entry = index_data.entry(addr.clone()).or_insert_with(|| {
                    new_indexes.insert(addr);
                    IndexData {
                        ty: real_type,
                        values: vec![],
                    }
                });

                if let Some(value) = value {
                    entry.values.push(value);
                } else {
                    entry.values.clear();
                }
            }

            Action::FlushFork => fork.flush(),
        }
    }

    let backup = db.merge_with_backup(fork.into_patch()).unwrap();
    // Check the new data in the DB.
    let new_snapshot = db.snapshot();
    for (addr, data) in &index_data {
        data.check(&new_snapshot, addr.to_owned())?;
    }
    // Check old data in the backup.
    for (addr, data) in &initial_data {
        data.check(&backup, addr.to_owned())?;
    }
    // Check aggregation in the backup.
    let system_schema = SystemSchema::new(&backup);
    let backup_state_hash = system_schema.state_hash();
    prop_assert_eq!(backup_state_hash, initial_state_hash);
    let backup_state_entries: HashMap<_, _> = system_schema.state_aggregator().iter().collect();
    prop_assert_eq!(&backup_state_entries, &initial_state_entries);
    for new_addr in &new_indexes {
        check_index_does_not_exist(&backup, new_addr.to_owned())?;
    }

    // Merge the backup into the DB and run the checks on the snapshot.
    db.merge(backup).unwrap();
    let snapshot = db.snapshot();
    let system_schema = SystemSchema::new(&snapshot);
    let backup_state_hash = system_schema.state_hash();
    prop_assert_eq!(backup_state_hash, initial_state_hash);
    let backup_state_entries: HashMap<_, _> = system_schema.state_aggregator().iter().collect();
    prop_assert_eq!(&backup_state_entries, &initial_state_entries);
    for new_addr in new_indexes {
        check_index_does_not_exist(&snapshot, new_addr)?;
    }

    Ok(())
}

#[test]
fn backup_with_honest_db_initialization() {
    let config = Config::with_cases(Config::default().cases / 4);
    let actions = vec(generate_action(), 1..ACTIONS_MAX_LEN);
    proptest!(config, |(init_actions in actions.clone(), main_actions in actions)| {
        let db = TemporaryDB::new();
        apply_actions(&db, init_actions, main_actions)?;
    });
}
