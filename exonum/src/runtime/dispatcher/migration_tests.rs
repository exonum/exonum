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

use exonum_crypto::gen_keypair;
use exonum_merkledb::{
    access::AccessExt, migration::Migration, HashTag, ObjectHash, SystemSchema, TemporaryDB,
};
use futures::IntoFuture;

use std::time::Duration;

use super::*;
use crate::{
    blockchain::{ApiSender, Block, BlockchainMut},
    helpers::ValidatorId,
    runtime::migrations::{InitMigrationError, MigrationError},
    runtime::{
        CallInfo, CoreError, DispatcherSchema, ErrorMatch, RuntimeIdentifier, WellKnownRuntime,
    },
};

const DELAY: Duration = Duration::from_millis(40);

#[derive(Debug)]
struct MigrationRuntime;

impl WellKnownRuntime for MigrationRuntime {
    const ID: u32 = 2;
}

impl Runtime for MigrationRuntime {
    fn deploy_artifact(
        &mut self,
        _artifact: ArtifactId,
        _deploy_spec: Vec<u8>,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
        Box::new(Ok(()).into_future())
    }

    fn is_artifact_deployed(&self, _id: &ArtifactId) -> bool {
        true
    }

    fn initiate_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _spec: &InstanceSpec,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn initiate_resuming_service(
        &self,
        _context: ExecutionContext<'_>,
        _spec: &InstanceSpec,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn update_service_status(
        &mut self,
        _snapshot: &dyn Snapshot,
        _spec: &InstanceSpec,
        _status: &InstanceStatus,
    ) {
    }

    fn migrate(
        &self,
        new_artifact: &ArtifactId,
        data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        let mut end_version = new_artifact.version.clone();
        end_version.patch = 0;

        let script = match new_artifact.name.as_str() {
            "good" => simple_delayed_migration,
            "complex" => {
                if *data_version < Version::new(0, 2, 0) {
                    end_version = Version::new(0, 2, 0);
                    complex_migration_part1
                } else if *data_version < Version::new(0, 3, 0) {
                    end_version = Version::new(0, 3, 0);
                    complex_migration_part2
                } else {
                    return Ok(None);
                }
            }
            "not-good" => erroneous_migration,
            "bad" => panicking_migration,
            "with-state" => migration_modifying_state_hash,
            "none" => return Ok(None),
            _ => return Err(InitMigrationError::NotSupported),
        };
        let script = MigrationScript::new(script, end_version);
        Ok(Some(script))
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        _call_info: &CallInfo,
        _arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn before_transactions(
        &self,
        _context: ExecutionContext<'_>,
        _instance_id: u32,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(
        &self,
        _context: ExecutionContext<'_>,
        _instance_id: u32,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {}
}

fn simple_delayed_migration(_ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    thread::sleep(DELAY);
    Ok(())
}

fn erroneous_migration(_ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    thread::sleep(DELAY);
    Err(MigrationError::new("This migration is unsuccessful!"))
}

fn panicking_migration(_ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    thread::sleep(DELAY);
    panic!("This migration is unsuccessful!");
}

fn migration_modifying_state_hash(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    for i in 1_u32..=2 {
        ctx.helper.new_data().get_proof_entry("entry").set(i);
        thread::sleep(DELAY / 2);
        ctx.helper.merge()?;
    }
    Ok(())
}

fn complex_migration_part1(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    assert!(ctx.data_version < Version::new(0, 2, 0));
    ctx.helper.new_data().get_proof_entry("entry").set(1_u32);
    Ok(())
}

fn complex_migration_part2(ctx: &mut MigrationContext) -> Result<(), MigrationError> {
    assert_eq!(ctx.data_version, Version::new(0, 2, 0));
    ctx.helper.new_data().get_proof_entry("entry").set(2_u32);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LocalResult {
    None,
    InMemory,
    Saved,
    SavedWithNodeRestart,
}

/// Test rig encapsulating typical tasks for migration tests, such as artifact deployment
/// and service instantiation.
#[derive(Debug)]
struct Rig {
    blockchain: BlockchainMut,
    next_service_id: InstanceId,
}

impl Rig {
    fn new() -> Self {
        let blockchain = Blockchain::new(TemporaryDB::new(), gen_keypair(), ApiSender::closed());
        let blockchain = blockchain
            .into_mut_with_dummy_config()
            .with_runtime(MigrationRuntime)
            .build();

        Self {
            blockchain,
            next_service_id: 100,
        }
    }

    /// Computes expected state hash of a migration.
    fn migration_hash(&self, indexes: &[(&str, Hash)]) -> Hash {
        let fork = self.blockchain.fork();
        let mut aggregator = fork.get_proof_map::<_, str, Hash>("_aggregator");
        for &(index_name, hash) in indexes {
            aggregator.put(index_name, hash);
        }
        aggregator.object_hash()
    }

    /// Emulates node stopping.
    fn stop(self) -> Blockchain {
        self.blockchain.immutable_view()
    }

    /// Emulates node restart by recreating the dispatcher.
    fn restart(&mut self) {
        let blockchain = self.blockchain.as_ref().to_owned();
        let blockchain = blockchain
            .into_mut_with_dummy_config()
            .with_runtime(MigrationRuntime)
            .build();
        self.blockchain = blockchain;
    }

    fn dispatcher(&mut self) -> &mut Dispatcher {
        self.blockchain.dispatcher()
    }

    fn migration_threads(&mut self) -> &HashMap<String, MigrationThread> {
        &self.dispatcher().migrations.threads
    }

    /// Asserts that no migration scripts are currently being executed.
    fn assert_no_migration_threads(&mut self) {
        assert!(self.migration_threads().is_empty());
    }

    /// Waits for migration scripts to finish according to the specified policy.
    fn wait_migration_threads(&mut self, local_result: LocalResult) {
        if local_result == LocalResult::None {
            // Don't wait at all.
        } else {
            // Wait for the script to finish.
            thread::sleep(DELAY * 3);
            if local_result == LocalResult::InMemory {
                // Keep the local result in memory.
            } else {
                self.create_block(self.blockchain.fork());
                assert!(self.dispatcher().migrations.threads.is_empty());

                if local_result == LocalResult::SavedWithNodeRestart {
                    self.restart();
                }
            }
        }
    }

    fn create_block(&mut self, fork: Fork) -> Block {
        let height = CoreSchema::new(&fork).next_height();
        let (block_hash, patch) = self.blockchain.create_patch_inner(
            fork,
            ValidatorId(0).into(),
            height,
            &[],
            &mut BTreeMap::new(),
        );
        self.blockchain
            .commit(patch, block_hash, vec![], &mut BTreeMap::new())
            .unwrap();
        self.blockchain.as_ref().last_block()
    }

    fn deploy_artifact(&mut self, name: &str, version: Version) -> ArtifactId {
        let artifact = ArtifactId {
            runtime_id: MigrationRuntime::ID,
            name: name.to_owned(),
            version,
        };

        let fork = self.blockchain.fork();
        Dispatcher::commit_artifact(&fork, artifact.clone(), vec![]);
        self.create_block(fork);
        artifact
    }

    fn initialize_service(&mut self, artifact: ArtifactId, name: &str) -> InstanceSpec {
        let service = InstanceSpec {
            artifact,
            id: self.next_service_id,
            name: name.to_owned(),
        };
        self.next_service_id += 1;

        let mut fork = self.blockchain.fork();
        let mut context = ExecutionContext::new(self.dispatcher(), &mut fork, Caller::Blockchain);
        context
            .initiate_adding_service(service.clone(), vec![])
            .expect("`initiate_adding_service` failed");
        self.create_block(fork);
        service
    }

    fn stop_service(&mut self, spec: &InstanceSpec) {
        let fork = self.blockchain.fork();
        Dispatcher::initiate_stopping_service(&fork, spec.id).unwrap();
        self.create_block(fork);
    }
}

/// Tests basic workflow of migration initiation.
#[test]
fn migration_workflow() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");

    // Since service is not stopped, the migration should fail.
    let fork = rig.blockchain.fork();
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::ServiceNotStopped));

    // Stop the service.
    rig.stop_service(&service);

    // Now, the migration start should succeed.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    // Migration scripts should not start executing immediately, but only on block commit.
    assert!(!rig
        .dispatcher()
        .migrations
        .threads
        .contains_key(&service.name));
    rig.create_block(fork);

    // Check that the migration was initiated.
    assert!(rig
        .dispatcher()
        .migrations
        .threads
        .contains_key(&service.name));

    // Create several more blocks before the migration is complete and check that
    // we don't spawn multiple migration scripts at once (this check is performed in `Migrations`).
    for _ in 0..3 {
        rig.create_block(rig.blockchain.fork());
    }

    // Wait until the migration script is completed and check that its result is recorded.
    thread::sleep(DELAY * 3);

    rig.create_block(rig.blockchain.fork());
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let state = schema.get_instance(service.id).unwrap();
    let end_version = match state.status.unwrap() {
        InstanceStatus::Migrating(migration) => migration.end_version,
        status => panic!("Unexpected service status: {:?}", status),
    };
    assert_eq!(end_version, Version::new(0, 5, 0));
    let res = schema.local_migration_result(&service.name).unwrap();
    assert_eq!(res.0, Ok(HashTag::empty_map_hash()));
    assert!(!rig
        .dispatcher()
        .migrations
        .threads
        .contains_key(&service.name));

    // Create couple more blocks to check that the migration script is not launched again,
    // and the migration result is not overridden (these checks are `debug_assert`s).
    for _ in 0..3 {
        rig.create_block(rig.blockchain.fork());
    }
    assert!(!rig
        .dispatcher()
        .migrations
        .threads
        .contains_key(&service.name));
}

/// Tests fast-forwarding a migration.
#[test]
fn fast_forward_migration() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("none", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("none", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "service");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);

    // Service version should be updated when the block is merged.
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let state = schema.get_instance(service.id).unwrap();
    assert_eq!(state.status, Some(InstanceStatus::Stopped));
    assert_eq!(state.pending_status, None);
    assert_eq!(state.data_version, Some(Version::new(0, 5, 2)));
}

/// Tests checks performed by the dispatcher during migration initiation.
#[test]
fn migration_immediate_errors() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let unrelated_artifact = rig.deploy_artifact("unrelated", "1.0.1".parse().unwrap());
    let old_service = rig.initialize_service(old_artifact.clone(), "old");
    rig.stop_service(&old_service);
    let new_service = rig.initialize_service(new_artifact.clone(), "new");
    rig.stop_service(&new_service);

    let fork = rig.blockchain.fork();

    // Attempt to upgrade service to an unrelated artifact.
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, unrelated_artifact.clone(), &old_service.name)
        .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::CannotUpgradeService));

    // Attempt to downgrade service.
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, old_artifact, &new_service.name)
        .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::CannotUpgradeService));

    // Attempt to migrate to the same version.
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &new_service.name)
        .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::CannotUpgradeService));

    // Attempt to migrate unknown service.
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, new_artifact, "bogus-service")
        .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));

    // Attempt to migrate to unknown artifact.
    let unknown_artifact = ArtifactId {
        runtime_id: RuntimeIdentifier::Rust as _,
        name: "good".to_owned(),
        version: Version::new(0, 6, 0),
    };
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, unknown_artifact.clone(), &old_service.name)
        .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::UnknownArtifactId));

    // Mark the artifact as pending.
    Dispatcher::commit_artifact(&fork, unknown_artifact.clone(), vec![]);
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, unknown_artifact, &old_service.name)
        .unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::ArtifactNotDeployed));
}

/// Tests that an unfinished migration script is restarted on node restart.
#[test]
fn migration_is_resumed_after_node_restart() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");
    rig.stop_service(&service);

    // Start migration.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);

    // Emulate node restart. Note that the old migration thread will continue running
    // as a detached thread, but since `Dispatcher.migrations` is dropped, the migration
    // will be aborted.
    rig.restart();
    assert!(rig.migration_threads().contains_key(&service.name));

    thread::sleep(DELAY * 3);
    rig.create_block(rig.blockchain.fork());
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let res = schema.local_migration_result(&service.name).unwrap();
    assert_eq!(res.0, Ok(HashTag::empty_map_hash()));
}

/// Tests that migration scripts are timely aborted on node stop.
#[test]
fn migration_threads_are_timely_aborted() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("with-state", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("with-state", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);

    thread::sleep(DELAY * 2 / 3);
    let blockchain = rig.stop();
    thread::sleep(DELAY * 10);
    let snapshot = blockchain.snapshot();
    let migration = Migration::new(&service.name, &snapshot);
    // The `migration_modifying_state_hash` script should complete the 0 or 1 merge, but not
    // 2 merges.
    let val = migration
        .get_proof_entry::<_, u32>("entry")
        .get()
        .unwrap_or(0);
    assert!(val < 2);

    // New merges should not be added with time.
    thread::sleep(DELAY * 2);
    let snapshot = blockchain.snapshot();
    let migration = Migration::new(&service.name, &snapshot);
    let new_val = migration
        .get_proof_entry::<_, u32>("entry")
        .get()
        .unwrap_or(0);
    assert_eq!(val, new_val);
}

/// Tests that a completed migration script is not launched again.
#[test]
fn completed_migration_is_not_resumed_after_node_restart() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");
    rig.stop_service(&service);

    // Start migration.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);

    thread::sleep(DELAY * 3);
    rig.create_block(rig.blockchain.fork());
    // Migration should be completed.
    rig.assert_no_migration_threads();
    // Check that the local migration result is persisted.
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert!(schema.local_migration_result(&service.name).is_some());

    // Therefore, the script should not resume after blockchain restart.
    rig.restart();
    rig.assert_no_migration_threads();
}

/// Tests that an error in a migration script is reflected in the local migration result.
fn test_erroneous_migration(artifact_name: &str) {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact(artifact_name, "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact(artifact_name, "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "service");
    rig.stop_service(&service);

    // Start migration.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);

    // Wait for the migration script to complete.
    let res = loop {
        thread::sleep(DELAY * 3);

        rig.create_block(rig.blockchain.fork());
        let snapshot = rig.blockchain.snapshot();
        let schema = DispatcherSchema::new(&snapshot);
        if let Some(res) = schema.local_migration_result(&service.name) {
            break res;
        }
    };
    assert!(res
        .0
        .unwrap_err()
        .contains("This migration is unsuccessful!"));
}

#[test]
fn migration_with_error() {
    test_erroneous_migration("not-good");
}

#[test]
fn migration_with_panic() {
    test_erroneous_migration("bad");
}

/// Tests that concurrent migrations with the same artifact are independent.
#[test]
fn concurrent_migrations_to_same_artifact() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "service");
    rig.stop_service(&service);
    let other_service = rig.initialize_service(old_artifact.clone(), "other-service");
    rig.stop_service(&other_service);
    let another_service = rig.initialize_service(old_artifact.clone(), "another-service");
    rig.stop_service(&another_service);

    // Place two migration starts in the same block.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &other_service.name)
        .unwrap();
    rig.create_block(fork);

    let threads = rig.migration_threads();
    assert!(threads.contains_key(&service.name));
    assert!(threads.contains_key(&other_service.name));
    assert!(!threads.contains_key(&another_service.name));

    // ...and one more in the following block.
    thread::sleep(DELAY * 2 / 3);
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &another_service.name)
        .unwrap();
    rig.create_block(fork);

    assert!(rig.migration_threads().contains_key(&another_service.name));

    // Wait for first two migrations to finish.
    thread::sleep(DELAY / 2);
    rig.create_block(rig.blockchain.fork());
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let res = schema.local_migration_result(&service.name).unwrap();
    assert_eq!(res.0, Ok(HashTag::empty_map_hash()));
    let res = schema.local_migration_result(&other_service.name).unwrap();
    assert_eq!(res.0, Ok(HashTag::empty_map_hash()));

    // Wait for the third migration to finish.
    thread::sleep(DELAY);
    rig.create_block(rig.blockchain.fork());
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let res = schema
        .local_migration_result(&another_service.name)
        .unwrap();
    assert_eq!(res.0, Ok(HashTag::empty_map_hash()));

    rig.assert_no_migration_threads();
}

/// Tests that migration workflow changes state hash as expected.
#[test]
fn migration_influencing_state_hash() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("with-state", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("with-state", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "service");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    let state_hash = rig.create_block(fork).state_hash;

    // Check that the state during migration does not influence the default `state_hash`.
    for _ in 0..2 {
        // The sleeping interval is chosen to be larger than the interval of DB merges
        // in the migration script.
        thread::sleep(DELAY * 2 / 3);

        let new_state_hash = rig.create_block(rig.blockchain.fork()).state_hash;
        assert_eq!(state_hash, new_state_hash);
    }

    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let res = schema.local_migration_result(&service.name).unwrap();
    let migration_hash = res.0.unwrap();

    let migration = Migration::new(&service.name, &snapshot);
    assert_eq!(migration_hash, migration.state_hash());
    let aggregator = migration.state_aggregator();
    assert_eq!(
        aggregator.keys().collect::<Vec<_>>(),
        vec!["service.entry".to_owned()]
    );
    assert_eq!(aggregator.get("service.entry"), Some(2_u32.object_hash()));
}

/// Tests the basic workflow of migration rollback.
#[test]
fn migration_rollback_workflow() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    rig.create_block(fork);

    // Wait until the migration is finished locally.
    thread::sleep(DELAY * 3);
    rig.create_block(rig.blockchain.fork());
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    schema.local_migration_result(&service.name).unwrap();
    rig.assert_no_migration_threads();

    // Signal the rollback.
    let fork = rig.blockchain.fork();
    Dispatcher::rollback_migration(&fork, &service.name).unwrap();
    rig.create_block(fork);

    // Check that local migration result is erased.
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert!(schema.local_migration_result(&service.name).is_none());
    let state = schema.get_instance(service.id).unwrap();
    assert_eq!(state.status, Some(InstanceStatus::Stopped));
    // The artifact version hasn't changed.
    assert_eq!(state.data_version, None);
}

/// Tests the checks performed by the dispatcher during migration rollback.
#[test]
fn migration_rollback_invariants() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");

    // Non-existing service.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::rollback_migration(&fork, "bogus").unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));

    // Service is not stopped.
    let err = Dispatcher::rollback_migration(&fork, &service.name).unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::NoMigration));

    rig.stop_service(&service);

    // Service is stopped, but there is no migration happening.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::rollback_migration(&fork, &service.name).unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::NoMigration));

    // Start migration and commit its result, thus making the rollback impossible.
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);
    let fork = rig.blockchain.fork();
    Dispatcher::commit_migration(&fork, &service.name, HashTag::empty_map_hash()).unwrap();

    // In the same block, we'll get an error because the service already has
    // a pending status update.
    let err = Dispatcher::rollback_migration(&fork, &service.name).unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::ServicePending));
    rig.create_block(fork);
    // ...In the next block, we'll get another error.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::rollback_migration(&fork, &service.name).unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::NoMigration));
}

/// Tests that migration rollback aborts locally executed migration script.
#[test]
fn migration_rollback_aborts_migration_script() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("with-state", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("with-state", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    rig.create_block(fork);

    // Rollback the migration without waiting for the migration script to succeed locally.
    let fork = rig.blockchain.fork();
    Dispatcher::rollback_migration(&fork, &service.name).unwrap();
    rig.create_block(fork);

    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert!(schema.local_migration_result(&service.name).is_none());
    rig.assert_no_migration_threads();
    let migration = Migration::new(&service.name, &snapshot);
    assert!(!migration.get_proof_entry::<_, u32>("entry").exists());

    // Wait some time to ensure that script doesn't merge changes to the DB.
    thread::sleep(DELAY);
    let snapshot = rig.blockchain.snapshot();
    let migration = Migration::new(&service.name, &snapshot);
    assert!(!migration.get_proof_entry::<_, u32>("entry").exists());
}

/// Tests that migration rollback erases data created by the migration script.
#[test]
fn migration_rollback_erases_migration_data() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("with-state", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("with-state", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    rig.create_block(fork);

    // Wait until the migration is finished locally.
    thread::sleep(DELAY * 10);
    rig.create_block(rig.blockchain.fork());
    let snapshot = rig.blockchain.snapshot();
    let migration = Migration::new(&service.name, &snapshot);
    assert_eq!(migration.get_proof_entry::<_, u32>("entry").get(), Some(2));

    let fork = rig.blockchain.fork();
    Dispatcher::rollback_migration(&fork, &service.name).unwrap();
    rig.create_block(fork);

    // Migration data should be dropped now.
    let snapshot = rig.blockchain.snapshot();
    let migration = Migration::new(&service.name, &snapshot);
    assert!(!migration.get_proof_entry::<_, u32>("entry").exists());
}

/// Tests basic migration commit workflow.
#[test]
fn migration_commit_workflow() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    rig.create_block(fork);

    // Wait until the migration is finished locally.
    thread::sleep(DELAY * 3);
    rig.create_block(rig.blockchain.fork());

    let fork = rig.blockchain.fork();
    Dispatcher::commit_migration(&fork, &service.name, HashTag::empty_map_hash()).unwrap();
    rig.create_block(fork);

    // Check that local migration result is erased.
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let res = schema.local_migration_result(&service.name).unwrap();
    assert_eq!(res.0.unwrap(), HashTag::empty_map_hash());
    let state = schema.get_instance(service.id).unwrap();
    let expected_status = InstanceStatus::migrating(InstanceMigration {
        target: new_artifact,
        end_version: Version::new(0, 5, 0),
        completed_hash: Some(HashTag::empty_map_hash()),
    });
    assert_eq!(state.status, Some(expected_status));
}

/// Tests checks performed by the dispatcher during migration commit.
#[test]
fn migration_commit_invariants() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "good");

    // Non-existing service.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::commit_migration(&fork, "bogus", Hash::zero()).unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::IncorrectInstanceId));

    // Service is not stopped.
    let err = Dispatcher::commit_migration(&fork, &service.name, Hash::zero()).unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::NoMigration));

    rig.stop_service(&service);

    // Service is stopped, but there is no migration happening.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::commit_migration(&fork, &service.name, Hash::zero()).unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::NoMigration));

    // Start migration and commit its result, making the second commit impossible.
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);
    let fork = rig.blockchain.fork();
    let migration_hash = HashTag::empty_map_hash();
    Dispatcher::commit_migration(&fork, &service.name, migration_hash).unwrap();

    // In the same block, we'll get an error because the service already has
    // a pending status update.
    let err = Dispatcher::commit_migration(&fork, &service.name, migration_hash).unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::ServicePending));
    rig.create_block(fork);
    // ...In the next block, we'll get another error.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::commit_migration(&fork, &service.name, migration_hash).unwrap_err();
    assert_eq!(err, ErrorMatch::from_fail(&CoreError::NoMigration));
}

/// Tests that a migration commit after the migration script finished locally with an error
/// leads to node stopping.
fn test_migration_commit_with_local_error(local_result: LocalResult) {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("not-good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("not-good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "service");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);

    rig.wait_migration_threads(local_result);

    let fork = rig.blockchain.fork();
    Dispatcher::commit_migration(&fork, &service.name, Hash::zero()).unwrap();
    rig.create_block(fork); // << should panic
}

#[test]
#[should_panic(expected = "locally it has finished with an error: This migration is unsuccessful")]
fn migration_commit_with_local_error_blocking() {
    test_migration_commit_with_local_error(LocalResult::None);
}

#[test]
#[should_panic(expected = "locally it has finished with an error: This migration is unsuccessful")]
fn migration_commit_with_local_error_in_memory() {
    test_migration_commit_with_local_error(LocalResult::InMemory);
}

#[test]
#[should_panic(expected = "locally it has finished with an error: This migration is unsuccessful")]
fn migration_commit_with_local_error_saved() {
    test_migration_commit_with_local_error(LocalResult::Saved);
}

#[test]
#[should_panic(expected = "locally it has finished with an error: This migration is unsuccessful")]
fn migration_commit_with_local_error_saved_and_node_restart() {
    test_migration_commit_with_local_error(LocalResult::SavedWithNodeRestart);
}

/// Tests that a migration commit after the migration script finished locally with another hash
/// leads to node stopping.
fn test_migration_commit_with_differing_hash(local_result: LocalResult) {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "service");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);

    rig.wait_migration_threads(local_result);

    let fork = rig.blockchain.fork();
    Dispatcher::commit_migration(&fork, &service.name, Hash::zero()).unwrap();
    rig.create_block(fork); // << should panic
}

#[test]
#[should_panic(expected = "locally it has finished with another hash")]
fn migration_commit_with_differing_hash_blocking() {
    test_migration_commit_with_differing_hash(LocalResult::None);
}

#[test]
#[should_panic(expected = "locally it has finished with another hash")]
fn migration_commit_with_differing_hash_in_memory() {
    test_migration_commit_with_differing_hash(LocalResult::InMemory);
}

#[test]
#[should_panic(expected = "locally it has finished with another hash")]
fn migration_commit_with_differing_hash_saved() {
    test_migration_commit_with_differing_hash(LocalResult::Saved);
}

#[test]
#[should_panic(expected = "locally it has finished with another hash")]
fn migration_commit_with_differing_hash_saved_and_node_restarted() {
    test_migration_commit_with_differing_hash(LocalResult::SavedWithNodeRestart);
}

/// Tests that committing a migration with a locally running migration script leads to the node
/// waiting until the script is completed.
#[test]
fn migration_commit_without_completing_script_locally() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("with-state", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("with-state", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "test");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    rig.create_block(fork);

    // Compute migration hash using the knowledge about the end state of migrated data.
    let migration_hash = rig.migration_hash(&[("test.entry", 2_u32.object_hash())]);

    let fork = rig.blockchain.fork();
    Dispatcher::commit_migration(&fork, &service.name, migration_hash).unwrap();
    rig.create_block(fork);
    // Check that the migration script has finished.
    rig.assert_no_migration_threads();

    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let state = schema.get_instance(service.id).unwrap();
    let expected_status = InstanceStatus::migrating(InstanceMigration {
        target: new_artifact,
        end_version: Version::new(0, 5, 0),
        completed_hash: Some(migration_hash),
    });
    assert_eq!(state.status, Some(expected_status));

    // Flush the migration.
    let mut fork = rig.blockchain.fork();
    Dispatcher::flush_migration(&mut fork, &service.name).unwrap();
    let state_hash = rig.create_block(fork).state_hash;

    // The artifact version should be updated.
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let state = schema.get_instance(service.id).unwrap();
    assert_eq!(state.data_version, Some(Version::new(0, 5, 0)));
    assert_eq!(state.status, Some(InstanceStatus::Stopped));
    assert!(schema.local_migration_result(&service.name).is_none());

    // Check that service data has been updated.
    let entry = snapshot.get_proof_entry::<_, u32>("test.entry");
    assert_eq!(entry.get(), Some(2));
    // Check state aggregation.
    let aggregator = SystemSchema::new(&snapshot).state_aggregator();
    assert_eq!(aggregator.get("test.entry"), Some(2_u32.object_hash()));
    assert_eq!(aggregator.object_hash(), state_hash);
}

/// Tests that the migration workflow is applicable to a migration spanning multiple scripts.
#[test]
fn two_part_migration() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("complex", "0.1.1".parse().unwrap());
    let new_artifact = rig.deploy_artifact("complex", "0.3.7".parse().unwrap());
    let service = rig.initialize_service(old_artifact, "test");
    rig.stop_service(&service);

    // First part of migration.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    rig.create_block(fork);

    let migration_hash = rig.migration_hash(&[("test.entry", 1_u32.object_hash())]);
    let fork = rig.blockchain.fork();
    Dispatcher::commit_migration(&fork, &service.name, migration_hash).unwrap();
    rig.create_block(fork);

    let mut fork = rig.blockchain.fork();
    Dispatcher::flush_migration(&mut fork, &service.name).unwrap();
    rig.create_block(fork);

    // Check service data and metadata.
    let snapshot = rig.blockchain.snapshot();
    assert_eq!(
        snapshot.get_proof_entry::<_, u32>("test.entry").get(),
        Some(1)
    );
    let schema = DispatcherSchema::new(&snapshot);
    let instance_state = schema.get_instance(service.id).unwrap();
    assert_eq!(instance_state.data_version, Some(Version::new(0, 2, 0)));

    // Second part of migration.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    rig.create_block(fork);

    let migration_hash = rig.migration_hash(&[("test.entry", 2_u32.object_hash())]);
    let fork = rig.blockchain.fork();
    Dispatcher::commit_migration(&fork, &service.name, migration_hash).unwrap();
    rig.create_block(fork);

    let mut fork = rig.blockchain.fork();
    Dispatcher::flush_migration(&mut fork, &service.name).unwrap();
    rig.create_block(fork);

    // Check service data and metadata.
    let snapshot = rig.blockchain.snapshot();
    assert_eq!(
        snapshot.get_proof_entry::<_, u32>("test.entry").get(),
        Some(2)
    );
    let schema = DispatcherSchema::new(&snapshot);
    let instance_state = schema.get_instance(service.id).unwrap();
    assert_eq!(instance_state.data_version, Some(Version::new(0, 3, 0)));
}
