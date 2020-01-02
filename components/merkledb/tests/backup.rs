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

//! Tests `DatabaseExt::merge_with_backup()`.

use proptest::{
    collection::vec,
    prop_assert, prop_assert_eq, prop_oneof, proptest, strategy,
    strategy::Strategy,
    test_runner::{Config, TestCaseResult},
};

use std::collections::{HashMap, HashSet};

use exonum_merkledb::{
    access::{Access, AccessExt},
    Database, DatabaseExt, IndexAddress, IndexType, SystemSchema, TemporaryDB,
};

mod work;
use self::work::*;

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

fn check_index_does_not_exist<S: Access>(snapshot: S, addr: IndexAddress) -> TestCaseResult {
    if let Some(index_type) = snapshot.index_type(addr) {
        prop_assert!(false, "{:?}", index_type);
    }
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

#[test]
fn backup_with_db_clearing() {
    let db = TemporaryDB::new();
    let actions = vec(generate_action(), 1..ACTIONS_MAX_LEN);
    proptest!(|(init_actions in actions.clone(), main_actions in actions)| {
        apply_actions(&db, init_actions, main_actions)?;
        db.clear().unwrap();
    });
}
