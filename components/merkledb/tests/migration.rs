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
    prop_assert_eq, prop_oneof, proptest, sample, strategy,
    strategy::Strategy,
    test_runner::{Config, TestCaseResult},
};

use std::collections::{HashMap, HashSet};

use exonum_merkledb::{
    access::{Access, AccessExt},
    migration::{flush_migration, rollback_migration, Migration},
    Database, HashTag, IndexAddress, IndexType, ObjectHash, Snapshot, SystemSchema, TemporaryDB,
};

mod work;
use self::work::*;

const ACTIONS_MAX_LEN: usize = 25;

const NAMESPACES: Strings = &["test", "other", "tes"];

const UNRELATED_NAMESPACES: Strings = &["other_", "unrelated"];

type Strings = &'static [&'static str];
type NewIndexes = HashMap<(&'static str, IndexAddress), IndexData>;

/// Constituent action applied to the DB during migration.
#[derive(Debug, Clone)]
enum MigrationAction {
    /// Do some work on a certain index. The index may be in the migration, or outside of it
    /// (including the case when the index will be replaced / removed by the migration).
    WorkOnIndex {
        /// Migration namespace. Empty for indexes outside a migration.
        namespace: &'static str,
        /// Index address.
        addr: IndexAddress,
        /// Type to initialize index to if it doesn't exist.
        index_type: IndexType,
        /// Value to insert into the index. If `None`, the index will be cleared instead.
        value: Option<Vec<u8>>,
    },
    /// Create a tombstone for the specified address.
    CreateTombstone {
        namespace: &'static str,
        addr: IndexAddress,
    },
    /// Roll back the specified migration.
    Rollback(&'static str),
    /// Flush the fork.
    FlushFork,
    /// Merge the fork into the DB.
    MergeFork,
}

/// Generates an atomic migration action.
///
/// `namespaces` denotes a list of namespaces in which migrations will be performed. Namespaces
/// should not intersect with `UNRELATED_NAMESPACES`.
fn generate_action(namespaces: Strings) -> impl Strategy<Value = MigrationAction> {
    let work_args = (
        sample::select(namespaces),
        generate_address(),
        generate_index_type(),
        generate_value(),
        bool::ANY,
    );
    let related_work =
        work_args.prop_map(|(namespace, addr, index_type, value, is_in_migration)| {
            if is_in_migration {
                MigrationAction::WorkOnIndex {
                    namespace,
                    addr,
                    index_type,
                    value,
                }
            } else {
                let addr = addr.prepend_name(namespace);
                MigrationAction::WorkOnIndex {
                    namespace: "",
                    addr,
                    index_type,
                    value,
                }
            }
        });

    let unrelated_work_args = (
        sample::select(UNRELATED_NAMESPACES),
        generate_address(),
        generate_index_type(),
        generate_value(),
    );
    let unrelated_work = unrelated_work_args.prop_map(|(ns, addr, index_type, value)| {
        let addr = addr.prepend_name(ns);
        MigrationAction::WorkOnIndex {
            namespace: "",
            addr,
            index_type,
            value,
        }
    });

    prop_oneof![
        related_work,
        unrelated_work,
        (sample::select(namespaces), generate_address())
            .prop_map(|(namespace, addr)| MigrationAction::CreateTombstone { namespace, addr }),
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

fn get_object_hash<S>(snapshot: &S, name: &str, index_type: IndexType) -> Hash
where
    S: Access,
{
    match index_type {
        IndexType::ProofEntry => snapshot.get_proof_entry::<_, ()>(name).object_hash(),
        IndexType::ProofList => snapshot.get_proof_list::<_, ()>(name).object_hash(),
        IndexType::ProofMap => snapshot.get_proof_map::<_, (), ()>(name).object_hash(),
        _ => unreachable!(),
    }
}

/// Checks the state of a particular state aggregator. `single_indexes` are the expected single
/// indexes in the DB within the `namespace`, together with their types.
fn check_namespace_aggregator<'a>(
    snapshot: &dyn Snapshot,
    namespace: &str,
    single_indexes: impl Iterator<Item = (&'a str, IndexType)>,
) -> TestCaseResult {
    let migration = Migration::new(namespace, snapshot);
    let aggregator = migration.state_aggregator();

    let mut expected_names = HashSet::new();
    for (name, index_type) in single_indexes {
        let aggregated_name = format!("{}.{}", namespace, name);
        let maybe_hash = if index_type.is_merkelized() {
            expected_names.insert(aggregated_name.clone());
            Some(get_object_hash(&migration, name, index_type))
        } else {
            None
        };
        prop_assert_eq!(aggregator.get(&aggregated_name), maybe_hash);
    }
    prop_assert_eq!(aggregator.keys().collect::<HashSet<_>>(), expected_names);
    Ok(())
}

fn check_default_aggregator<'a>(
    snapshot: &dyn Snapshot,
    single_indexes: impl Iterator<Item = (&'a str, IndexType)>,
) -> TestCaseResult {
    let aggregator = SystemSchema::new(snapshot).state_aggregator();

    let mut expected_names = HashSet::new();
    for (name, index_type) in single_indexes {
        let maybe_hash = if index_type.is_merkelized() {
            expected_names.insert(name.to_owned());
            Some(get_object_hash(&snapshot, name, index_type))
        } else {
            None
        };
        prop_assert_eq!(aggregator.get(name), maybe_hash);
    }
    prop_assert_eq!(aggregator.keys().collect::<HashSet<_>>(), expected_names);
    Ok(())
}

fn single_indexes<'a>(
    indexes: &'a NewIndexes,
    namespace: &'static str,
) -> impl Iterator<Item = (&'a str, IndexType)> {
    indexes.iter().filter_map(move |((ns, addr), data)| {
        if addr.id_in_group().is_none() && *ns == namespace {
            Some((addr.name(), data.ty))
        } else {
            None
        }
    })
}

fn check_intermediate_consistency(
    snapshot: &dyn Snapshot,
    namespaces: Strings,
    old_single_indexes: &HashMap<String, IndexType>,
    new_indexes: &NewIndexes,
) -> TestCaseResult {
    check_default_aggregator(
        snapshot,
        old_single_indexes
            .iter()
            .map(|(name, ty)| (name.as_str(), *ty)),
    )?;

    for &namespace in namespaces {
        let indexes = single_indexes(new_indexes, namespace);
        check_namespace_aggregator(snapshot, namespace, indexes)?;
    }
    for ((ns, addr), data) in new_indexes {
        let migration = Migration::new(*ns, snapshot);
        data.check(migration, addr.to_owned())?;
    }
    Ok(())
}

fn check_final_consistency(
    snapshot: &dyn Snapshot,
    namespaces: Strings,
    aggregated_indexes: &HashMap<String, IndexType>,
    new_indexes: &HashMap<IndexAddress, IndexData>,
) -> TestCaseResult {
    for (addr, data) in new_indexes {
        data.check(snapshot, addr.to_owned())?;
    }
    check_default_aggregator(
        snapshot,
        aggregated_indexes
            .iter()
            .map(|(name, ty)| (name.as_str(), *ty)),
    )?;

    for &namespace in namespaces {
        let ns_aggregator = Migration::new(namespace, snapshot).state_aggregator();
        prop_assert_eq!(ns_aggregator.keys().count(), 0);
    }

    for (name, ty) in aggregated_indexes {
        if *ty == IndexType::Tombstone {
            // The index should be fully removed; thus, creating a `ProofMapIndex` on its place
            // should succeed and it should have a default `object_hash`.
            prop_assert_eq!(
                get_object_hash(&snapshot, name, IndexType::ProofMap),
                HashTag::empty_map_hash()
            );
        }
    }

    Ok(())
}

fn apply_actions(
    db: &TemporaryDB,
    actions: Vec<MigrationAction>,
    namespaces: Strings,
) -> TestCaseResult {
    // Original single indexes together with their type.
    let mut original_indexes = HashMap::new();
    // All indexes in the migration together with type and expected contents.
    let mut new_indexes: NewIndexes = HashMap::new();

    let mut fork = db.fork();
    for action in actions {
        match action {
            MigrationAction::WorkOnIndex {
                namespace,
                addr,
                index_type,
                value,
            } => {
                let is_in_group = addr.id_in_group().is_some();
                let real_type = if namespace.is_empty() {
                    work_on_index(&fork, addr.clone(), index_type, value.clone())
                } else {
                    let migration = Migration::new(namespace, &fork);
                    work_on_index(migration.clone(), addr.clone(), index_type, value.clone())
                };

                if !namespace.is_empty() {
                    let entry = new_indexes
                        .entry((namespace, addr))
                        .or_insert_with(|| IndexData {
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

            MigrationAction::CreateTombstone { namespace, addr } => {
                let migration = Migration::new(namespace, &fork);
                if migration.index_type(addr.clone()).is_none() {
                    migration.create_tombstone(addr.clone());
                    new_indexes.insert(
                        (namespace, addr),
                        IndexData {
                            ty: IndexType::Tombstone,
                            values: vec![],
                        },
                    );
                }
            }

            MigrationAction::Rollback(namespace) => {
                rollback_migration(&mut fork, namespace);
                new_indexes.retain(|(ns, _), _| *ns != namespace);
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
        flush_migration(&mut fork, namespace);
    }

    // Compute the final list of indexes. Note that indexes removed in the migration
    // will have `Tombstone` type.
    let new_indexes: HashMap<_, _> = new_indexes
        .into_iter()
        .map(|((ns, addr), data)| {
            let new_addr = addr.prepend_name(ns);
            (new_addr, data)
        })
        .collect();

    let mut aggregated_indexes = original_indexes;
    aggregated_indexes.extend(new_indexes.iter().filter_map(|(addr, data)| {
        if addr.id_in_group().is_none() {
            Some((addr.name().to_owned(), data.ty))
        } else {
            None
        }
    }));

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
        db.clear().unwrap();
    });
}

#[test]
fn single_migration_with_rollbacks() {
    const SINGLE_NAMESPACE: Strings = &["test"];
    let db = TemporaryDB::new();
    let action = generate_action_with_rollbacks(SINGLE_NAMESPACE);
    proptest!(|(actions in vec(action, 1..ACTIONS_MAX_LEN))| {
        apply_actions(&db, actions, SINGLE_NAMESPACE)?;
        db.clear().unwrap();
    });
}

#[test]
fn multiple_migrations_with_synced_end() {
    let db = TemporaryDB::new();
    proptest!(|(actions in vec(generate_action(NAMESPACES), 1..ACTIONS_MAX_LEN))| {
        apply_actions(&db, actions, NAMESPACES)?;
        db.clear().unwrap();
    });
}

#[test]
fn multiple_migrations_with_synced_end_and_rollbacks() {
    let db = TemporaryDB::new();
    let action = generate_action_with_rollbacks(NAMESPACES);
    proptest!(|(actions in vec(action, 1..ACTIONS_MAX_LEN))| {
        apply_actions(&db, actions, NAMESPACES)?;
        db.clear().unwrap();
    });
}
