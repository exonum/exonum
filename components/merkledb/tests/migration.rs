//! This test checks that migration works properly:
//!
//! - Migrated indexes are properly aggregated during and after migration
//! - Migrated data is correctly added / replaced / removed after merge
//! - Migration rollbacks work properly
//!
//! **NB.** For performance, some tests initialize the database outside the test. This should
//! be fine if the test passes, but can lead to weird errors if it fails. In this case,
//! move database initialization inside the test to extract the sequence of actions failing the test.

use exonum_crypto::Hash;
use proptest::{
    bool,
    collection::vec,
    option, prop_assert_eq, prop_oneof, proptest, sample, strategy,
    strategy::Strategy,
    test_runner::{Config, TestCaseResult},
};

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    iter::FromIterator,
};

use exonum_merkledb::{
    access::{AccessErrorKind, AccessExt},
    Database, Fork, HashTag, IndexAddress, IndexType, ObjectHash, Snapshot, SystemSchema,
    TemporaryDB,
};

const ACTIONS_MAX_LEN: usize = 25;

type Strings = &'static [&'static str];

const NAMESPACES: Strings = &["test", "other", "tes"];

const UNRELATED_NAMESPACES: Strings = &["other_", "unrelated"];

const INDEX_NAMES: Strings = &[
    "foo",
    "bar",
    "b",
    "overly_long_prefix_still_should_work_though",
];

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
    /// Roll back the specified migration.
    Rollback(&'static str),
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
fn generate_name(namespaces: Strings) -> impl Strategy<Value = IndexAddress> + Clone {
    let index_name = sample::select(INDEX_NAMES);
    (sample::select(namespaces), index_name).prop_map(|(namespace, index_name)| {
        IndexAddress::from_root(namespace).append_name(index_name)
    })
}

/// Generates an `IndexAddress` optionally placed in a group.
fn generate_address(
    name: impl Strategy<Value = IndexAddress> + Clone,
) -> impl Strategy<Value = IndexAddress> {
    prop_oneof![
        // Non-prefixed addresses
        name.clone(),
        // Prefixed addresses
        (name, 1_u8..8).prop_map(|(addr, prefix)| addr.append_key(&prefix)),
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

/// Converts the provided address into its migration counterpart.
fn migration_addr(addr: &IndexAddress) -> IndexAddress {
    let mut new_addr = IndexAddress::from_root(format!("^{}", addr.name()));
    if let Some(bytes) = addr.id_in_group() {
        new_addr = new_addr.append_key(bytes);
    }
    new_addr
}

/// Generates an atomic migration action.
///
/// `namespaces` denotes a list of namespaces in which migrations will be performed. Namespaces
/// should not intersect with `UNRELATED_NAMESPACES`.
fn generate_action(namespaces: Strings) -> impl Strategy<Value = MigrationAction> {
    let work_args = (
        generate_address(generate_name(namespaces)),
        generate_index_type(),
        generate_value(),
        bool::ANY,
    );
    let unrelated_work_args = (
        generate_address(generate_name(UNRELATED_NAMESPACES)),
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
        generate_address(generate_name(namespaces))
            .prop_map(|addr| { MigrationAction::CreateTombstone(migration_addr(&addr)) }),
        strategy::Just(MigrationAction::FlushFork),
        strategy::Just(MigrationAction::MergeFork),
    ]
}

fn generate_action_with_rollbacks(namespaces: Strings) -> impl Strategy<Value = MigrationAction> {
    prop_oneof![
        9 => generate_action(namespaces),
        1 => sample::select(namespaces).prop_map(MigrationAction::Rollback),
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

#[derive(Debug)]
struct IndexData {
    namespace: &'static str,
    ty: IndexType,
    values: Vec<Vec<u8>>,
}

/// Checks the state of a particular state aggregator. `single_indexes` are the expected single
/// indexes in the DB within the `namespace`, together with their types.
fn check_aggregator<'a>(
    snapshot: &dyn Snapshot,
    namespace: &str,
    single_indexes: impl Iterator<Item = (&'a str, IndexType)>,
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
            Some(get_object_hash(snapshot, name, index_type))
        } else {
            None
        };
        assert_eq!(aggregator.get(aggregated_name), maybe_hash);
    }
    prop_assert_eq!(aggregator.keys().collect::<HashSet<_>>(), expected_names);
    Ok(())
}

fn single_indexes<'a>(
    indexes: &'a HashMap<IndexAddress, IndexData>,
    namespace: &'static str,
) -> impl Iterator<Item = (&'a str, IndexType)> {
    indexes.iter().filter_map(move |(addr, data)| {
        if addr.id_in_group().is_none() && data.namespace == namespace {
            Some((addr.name(), data.ty))
        } else {
            None
        }
    })
}

fn check_contents(
    snapshot: &dyn Snapshot,
    new_indexes: &HashMap<IndexAddress, IndexData>,
) -> TestCaseResult {
    for (addr, data) in new_indexes {
        let addr = addr.to_owned();
        match data.ty {
            IndexType::Entry => {
                let val = snapshot.get_entry::<_, Vec<u8>>(addr).get();
                prop_assert_eq!(val.as_ref(), data.values.last());
            }
            IndexType::ProofEntry => {
                let val = snapshot.get_proof_entry::<_, Vec<u8>>(addr).get();
                prop_assert_eq!(val.as_ref(), data.values.last());
            }

            IndexType::List => {
                let list = snapshot.get_list::<_, Vec<u8>>(addr);
                prop_assert_eq!(list.len(), data.values.len() as u64);
                let values = list.iter().collect::<Vec<_>>();
                prop_assert_eq!(&values, &data.values);
            }
            IndexType::ProofList => {
                let list = snapshot.get_proof_list::<_, Vec<u8>>(addr);
                prop_assert_eq!(list.len(), data.values.len() as u64);
                let values = list.iter().collect::<Vec<_>>();
                prop_assert_eq!(&values, &data.values);
            }

            IndexType::Map => {
                let map = snapshot.get_map::<_, u8, Vec<u8>>(addr);
                let expected_map =
                    BTreeMap::from_iter(data.values.iter().map(|val| (val[0], val.clone())));
                // Using `Vec<_>` allows to test for duplicate entries during iteration etc.
                let expected_map: Vec<_> = expected_map.into_iter().collect();
                assert_eq!(map.iter().collect::<Vec<_>>(), expected_map);
            }
            IndexType::ProofMap => {
                let map = snapshot.get_proof_map::<_, u8, Vec<u8>>(addr);
                let expected_map =
                    BTreeMap::from_iter(data.values.iter().map(|val| (val[0], val.clone())));
                let expected_map: Vec<_> = expected_map.into_iter().collect();
                assert_eq!(map.iter().collect::<Vec<_>>(), expected_map);
            }

            _ => {}
        }
    }
    Ok(())
}

fn check_intermediate_consistency(
    snapshot: &dyn Snapshot,
    namespaces: Strings,
    old_single_indexes: &HashMap<String, IndexType>,
    new_indexes: &HashMap<IndexAddress, IndexData>,
) -> TestCaseResult {
    check_aggregator(
        snapshot,
        "",
        old_single_indexes
            .iter()
            .map(|(name, ty)| (name.as_str(), *ty)),
    )?;

    for &namespace in namespaces {
        let indexes = single_indexes(new_indexes, namespace);
        check_aggregator(snapshot, namespace, indexes)?;
    }
    check_contents(snapshot, new_indexes)?;
    Ok(())
}

fn check_final_consistency(
    snapshot: &dyn Snapshot,
    namespaces: Strings,
    aggregated_indexes: &HashMap<String, IndexType>,
    new_indexes: &HashMap<IndexAddress, IndexData>,
) -> TestCaseResult {
    check_contents(snapshot, new_indexes)?;
    check_aggregator(
        snapshot,
        "",
        aggregated_indexes
            .iter()
            .map(|(name, ty)| (name.as_str(), *ty)),
    )?;

    let system_schema = SystemSchema::new(snapshot);
    for &namespace in namespaces {
        let ns_aggregator = system_schema.namespace_state_aggregator(namespace);
        prop_assert_eq!(ns_aggregator.keys().count(), 0);
    }

    for (name, ty) in aggregated_indexes {
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
fn get_single_indexes(snapshot: &dyn Snapshot, namespaces: Strings) -> HashMap<String, IndexType> {
    let mut indexes = HashMap::new();

    for &namespace in namespaces.iter().chain(UNRELATED_NAMESPACES) {
        for &index_name in INDEX_NAMES {
            let addr = IndexAddress::from_root(namespace).append_name(index_name);
            let full_name = addr.name().to_owned();
            if let Err(e) = snapshot.touch_index(addr, IndexType::Unknown) {
                if let AccessErrorKind::WrongIndexType { actual, .. } = e.kind {
                    indexes.insert(full_name, actual);
                }
            }
        }
    }
    indexes
}

/// Gets a namespaces from an address.
fn get_namespace(addr: &IndexAddress, namespaces: Strings) -> &'static str {
    namespaces
        .iter()
        .find(|&&ns| {
            let name = addr.name();
            let ns_end = ns.len() + 1;
            &name[1..ns_end] == ns && &name[ns_end..=ns_end] == "."
        })
        .expect("Index not in the namespace")
}

fn apply_actions(
    db: &TemporaryDB,
    actions: Vec<MigrationAction>,
    namespaces: Strings,
) -> TestCaseResult {
    // Original single indexes together with their type.
    let mut original_indexes = get_single_indexes(&db.snapshot(), namespaces);
    // All indexes in the migration together with type and expected contents.
    let mut new_indexes = HashMap::new();

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
                let real_type =
                    MigrationAction::work(&fork, addr.clone(), index_type, value.clone());
                if is_in_migration {
                    let namespace = get_namespace(&addr, namespaces);
                    let entry = new_indexes.entry(addr).or_insert_with(|| IndexData {
                        namespace,
                        ty: real_type,
                        values: vec![],
                    });

                    if let Some(value) = value {
                        entry.values.push(value);
                    } else {
                        entry.values.clear();
                    }
                } else if !is_in_group {
                    original_indexes.insert(addr.name().to_owned(), real_type);
                }
            }

            MigrationAction::CreateTombstone(addr) => {
                if fork.touch_index(addr.clone(), IndexType::Tombstone).is_ok() {
                    let namespace = get_namespace(&addr, namespaces);
                    new_indexes.insert(
                        addr,
                        IndexData {
                            namespace,
                            ty: IndexType::Tombstone,
                            values: vec![],
                        },
                    );
                }
            }

            MigrationAction::Rollback(namespace) => {
                fork.rollback_migration(namespace);
                new_indexes.retain(|_, data| data.namespace != namespace);
            }

            MigrationAction::FlushFork => {
                fork.flush();
            }
            MigrationAction::MergeFork => {
                let patch = fork.into_patch();
                check_intermediate_consistency(
                    &patch,
                    namespaces,
                    &original_indexes,
                    &new_indexes,
                )?;
                db.merge(patch).unwrap();
                fork = db.fork();
            }
        }
    }

    for &namespace in namespaces {
        fork.flush_migration(namespace);
    }

    // Compute the final list of indexes. Note that indexes removed in the migration
    // will have `Tombstone` type.
    new_indexes = new_indexes
        .into_iter()
        .map(|(addr, data)| {
            // Remove '^' prefix from the address.
            let mut new_addr = IndexAddress::from_root(&addr.name()[1..]);
            if let Some(prefix) = addr.id_in_group() {
                new_addr = new_addr.append_key(prefix);
            }
            (new_addr, data)
        })
        .collect();
    let mut aggregated_indexes = original_indexes;
    for &namespace in namespaces {
        aggregated_indexes.extend(
            single_indexes(&new_indexes, namespace).map(|(name, ty)| (name.to_owned(), ty)),
        );
    }

    let patch = fork.into_patch();
    check_final_consistency(&patch, namespaces, &aggregated_indexes, &new_indexes)?;
    db.merge(patch).unwrap();
    let snapshot = db.snapshot();
    check_final_consistency(&snapshot, namespaces, &aggregated_indexes, &new_indexes)?;

    Ok(())
}

#[test]
fn single_migration_with_honest_db_initialization() {
    const SINGLE_NAMESPACE: Strings = &["test"];
    let config = Config::with_cases(Config::default().cases / 4);

    proptest!(config, |(actions in vec(generate_action(SINGLE_NAMESPACE), 1..ACTIONS_MAX_LEN))| {
        let db = TemporaryDB::new();
        apply_actions(&db, actions, SINGLE_NAMESPACE)?;
    });
}

/// All migration actions are in a single namespace `test`.
#[test]
fn single_migration() {
    const SINGLE_NAMESPACE: Strings = &["test"];
    let db = TemporaryDB::new();
    proptest!(|(actions in vec(generate_action(SINGLE_NAMESPACE), 1..ACTIONS_MAX_LEN))| {
        apply_actions(&db, actions, SINGLE_NAMESPACE)?;
    });
}

#[test]
fn single_migration_with_rollbacks() {
    const SINGLE_NAMESPACE: Strings = &["test"];
    let db = TemporaryDB::new();
    let action = generate_action_with_rollbacks(SINGLE_NAMESPACE);
    proptest!(|(actions in vec(action, 1..ACTIONS_MAX_LEN))| {
        apply_actions(&db, actions, SINGLE_NAMESPACE)?;
    });
}

#[test]
fn multiple_migrations_with_synced_end() {
    let db = TemporaryDB::new();
    proptest!(|(actions in vec(generate_action(NAMESPACES), 1..ACTIONS_MAX_LEN))| {
        apply_actions(&db, actions, NAMESPACES)?;
    });
}

#[test]
fn multiple_migrations_with_synced_end_and_rollbacks() {
    let db = TemporaryDB::new();
    let action = generate_action_with_rollbacks(NAMESPACES);
    proptest!(|(actions in vec(action, 1..ACTIONS_MAX_LEN))| {
        apply_actions(&db, actions, NAMESPACES)?;
    });
}
