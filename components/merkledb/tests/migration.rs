//! This test checks that migration works properly:
//!
//! - Migrated indexes are properly aggregated during and after migration
//! - Removed indexes are properly cleaned up

use exonum_crypto::Hash;
use proptest::{
    bool,
    collection::vec,
    prop_assert_eq, prop_oneof, proptest, strategy,
    strategy::Strategy,
    test_runner::{Config, TestCaseResult},
};

use std::collections::{HashMap, HashSet};

use exonum_merkledb::{
    access::{AccessErrorKind, AccessExt},
    Database, Fork, HashTag, IndexAddress, IndexType, ObjectHash, Snapshot, SystemSchema,
    TemporaryDB,
};

const ACTIONS_MAX_LEN: usize = 20;

/// Constituent action applied to the DB during migration.
#[derive(Debug, Clone)]
enum MigrationAction {
    /// Do some work on a certain index. The index may be in the migration, or outside of it
    /// (including the case when the index will be replaced / removed by the migration).
    WorkOnIndex {
        /// Index address.
        addr: IndexAddress,
        /// Type to initialize index to if it doesn't exist.
        index_type: IndexType,
        /// Value to insert into the index. If `None`, the index will be cleared instead.
        value: Option<Vec<u8>>,
    },
    /// Create a tombstone for the specified address.
    CreateTombstone(IndexAddress),
    /// Flush the fork.
    FlushFork,
    /// Merge the fork into the DB.
    MergeFork,
}

impl MigrationAction {
    fn work(
        fork: &Fork,
        addr: IndexAddress,
        mut index_type: IndexType,
        value: Option<Vec<u8>>,
    ) -> IndexType {
        if let Err(e) = fork.touch_index(addr.clone(), index_type) {
            if let AccessErrorKind::WrongIndexType { actual, .. } = e.kind {
                index_type = actual;
            }
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
}

/// Generates a name for an index participating in migration.
fn generate_name() -> impl Strategy<Value = IndexAddress> {
    let names = prop_oneof![
        strategy::Just("test.foo"),
        strategy::Just("test.bar"),
        strategy::Just("test.baz"),
        strategy::Just("test.buzz"),
    ];
    names.prop_map(IndexAddress::from_root)
}

/// Generates a name for an index **not** participating in migration.
fn generate_unrelated_name() -> impl Strategy<Value = IndexAddress> {
    let names = prop_oneof![
        strategy::Just("test_.foo"),
        strategy::Just("tes.bar"),
        strategy::Just("unrelated.baz"),
    ];
    names.prop_map(IndexAddress::from_root)
}

/// Generates an `IndexAddress` optionally placed in a group.
fn generate_address<T: Strategy<Value = IndexAddress>>(
    name: fn() -> T,
) -> impl Strategy<Value = IndexAddress> {
    prop_oneof![
        // Non-prefixed addresses
        name(),
        // Prefixed addresses
        (name(), 1_u8..8).prop_map(|(addr, prefix)| addr.append_key(&prefix)),
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
    prop_oneof![strategy::Just(None), vec(0_u8..4, 1..=1).prop_map(Some)]
}

/// Converts the provided address into its migration counterpart.
fn migration_addr(addr: &IndexAddress) -> IndexAddress {
    let mut new_addr = IndexAddress::from_root(format!("^{}", addr.name()));
    if let Some(bytes) = addr.id_in_group() {
        new_addr = new_addr.append_key(bytes);
    }
    new_addr
}

fn generate_action() -> impl Strategy<Value = MigrationAction> {
    let work_args = (
        generate_address(generate_name),
        generate_index_type(),
        generate_value(),
        bool::ANY,
    );
    let unrelated_work_args = (
        generate_address(generate_unrelated_name),
        generate_index_type(),
        generate_value(),
    );

    prop_oneof![
        work_args.prop_map(|(mut addr, index_type, value, is_in_migration)| {
            if is_in_migration {
                addr = migration_addr(&addr);
            }

            MigrationAction::WorkOnIndex {
                addr,
                index_type,
                value,
            }
        }),
        unrelated_work_args.prop_map(|(addr, index_type, value)| MigrationAction::WorkOnIndex {
            addr,
            index_type,
            value,
        }),
        generate_address(generate_name)
            .prop_map(|addr| { MigrationAction::CreateTombstone(migration_addr(&addr)) }),
        strategy::Just(MigrationAction::FlushFork),
        strategy::Just(MigrationAction::MergeFork),
    ]
}

fn get_object_hash(snapshot: &dyn Snapshot, name: &str, index_type: IndexType) -> Hash {
    match index_type {
        IndexType::ProofEntry => snapshot.get_proof_entry::<_, ()>(name).object_hash(),
        IndexType::ProofList => snapshot.get_proof_list::<_, ()>(name).object_hash(),
        IndexType::ProofMap => snapshot.get_proof_map::<_, (), ()>(name).object_hash(),
        _ => unreachable!(),
    }
}

/// Checks the state of a particular state aggregator. `single_indexes` are the expected single
/// indexes in the DB aggregated within the `namespace`, together with their types.
fn check_aggregator(
    snapshot: &dyn Snapshot,
    namespace: &str,
    single_indexes: &HashMap<String, IndexType>,
) -> TestCaseResult {
    let aggregator = SystemSchema::new(snapshot).namespace_state_aggregator(namespace);
    let mut expected_names = HashSet::new();
    for (name, index_type) in single_indexes {
        let aggregated_name = if name.starts_with('^') {
            &name[1..]
        } else {
            name
        };
        let maybe_hash = if index_type.is_merkelized() {
            expected_names.insert(aggregated_name.to_owned());
            Some(get_object_hash(snapshot, name, *index_type))
        } else {
            None
        };
        assert_eq!(aggregator.get(aggregated_name), maybe_hash);
    }
    prop_assert_eq!(aggregator.keys().collect::<HashSet<_>>(), expected_names);
    Ok(())
}

fn check_intermediate_consistency(
    snapshot: &dyn Snapshot,
    old_single_indexes: &HashMap<String, IndexType>,
    new_single_indexes: &HashMap<String, IndexType>,
) -> TestCaseResult {
    check_aggregator(snapshot, "", old_single_indexes)?;
    check_aggregator(snapshot, "test", new_single_indexes)?;
    Ok(())
}

fn check_final_consistency(
    snapshot: &dyn Snapshot,
    single_indexes: &HashMap<String, IndexType>,
) -> TestCaseResult {
    check_aggregator(snapshot, "", single_indexes)?;

    let system_schema = SystemSchema::new(snapshot);
    let ns_aggregator = system_schema.namespace_state_aggregator("test");
    prop_assert_eq!(ns_aggregator.keys().count(), 0);

    for (name, ty) in single_indexes {
        if *ty == IndexType::Tombstone {
            // The index should be fully removed, thus creating a `ProofMapIndex` on its place
            // should succeed and it should have a default `object_hash`.
            prop_assert_eq!(
                get_object_hash(snapshot, name, IndexType::ProofMap),
                HashTag::empty_map_hash()
            );
        }
    }

    Ok(())
}

/// Gets the single indexes and their types from the `snapshot`. This uses the fact that only
/// a limited amount of indexes may be generated by `MigrationAction`s.
fn get_single_indexes(snapshot: &dyn Snapshot) -> HashMap<String, IndexType> {
    const POSSIBLE_NAMES: &[&str] = &[
        "test.foo",
        "test.bar",
        "test.baz",
        "test.buzz",
        "test_.foo",
        "tes.bar",
        "unrelated.baz",
    ];

    let mut indexes = HashMap::new();
    for &name in POSSIBLE_NAMES {
        if let Err(e) = snapshot.touch_index(name, IndexType::Unknown) {
            if let AccessErrorKind::WrongIndexType { actual, .. } = e.kind {
                indexes.insert(name.to_owned(), actual);
            }
        }
    }
    indexes
}

fn apply_actions(db: &TemporaryDB, actions: Vec<MigrationAction>) -> TestCaseResult {
    let mut old_aggregation = get_single_indexes(&db.snapshot());
    let mut new_aggregation = HashMap::new();
    let mut fork = db.fork();

    for action in actions {
        match action {
            MigrationAction::WorkOnIndex {
                addr,
                index_type,
                value,
            } => {
                let is_in_group = addr.id_in_group().is_some();
                let is_in_migration = &addr.name()[0..1] == "^";
                let real_type = MigrationAction::work(&fork, addr.clone(), index_type, value);
                if !is_in_group {
                    let aggregation = if is_in_migration {
                        &mut new_aggregation
                    } else {
                        &mut old_aggregation
                    };
                    aggregation.insert(addr.name().to_owned(), real_type);
                }
            }
            MigrationAction::CreateTombstone(addr) => {
                let is_in_group = addr.id_in_group().is_some();
                let index_name = addr.name().to_owned();
                if fork.touch_index(addr, IndexType::Tombstone).is_ok() && !is_in_group {
                    new_aggregation.insert(index_name, IndexType::Tombstone);
                }
            }
            MigrationAction::FlushFork => {
                fork.flush();
            }
            MigrationAction::MergeFork => {
                let patch = fork.into_patch();
                check_intermediate_consistency(&patch, &old_aggregation, &new_aggregation)?;
                db.merge(patch).unwrap();
                fork = db.fork();
            }
        }
    }

    fork.finish_migration("test");

    // Compute the final list of indexes. Note that indexes removed in the migration
    // will have `Tombstone` type.
    let mut aggregation = old_aggregation;
    aggregation.extend(
        new_aggregation
            .into_iter()
            .map(|(name, ty)| (name[1..].to_owned(), ty)),
    );

    let patch = fork.into_patch();
    check_final_consistency(&patch, &aggregation)?;
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    check_final_consistency(&snapshot, &aggregation)?;

    Ok(())
}

#[test]
fn migration_works_with_honest_db_initialization() {
    let config = Config::with_cases(Config::default().cases / 4);
    proptest!(config, |(actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
        let db = TemporaryDB::new();
        apply_actions(&db, actions)?;
    });
}

#[test]
fn migration_works_with_shared_db() {
    let db = TemporaryDB::new();
    proptest!(|(actions in vec(generate_action(), 1..ACTIONS_MAX_LEN))| {
        apply_actions(&db, actions)?;
    });
}
