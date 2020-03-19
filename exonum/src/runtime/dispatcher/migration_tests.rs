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

use assert_matches::assert_matches;
use exonum_crypto::KeyPair;
use exonum_merkledb::{
    access::{AccessExt, CopyAccessExt},
    migration::Migration,
    HashTag, ObjectHash, SystemSchema, TemporaryDB,
};

use std::time::Duration;

use super::*;
use crate::{
    blockchain::{ApiSender, Block, BlockParams, BlockchainMut},
    helpers::{Height, ValidatorId},
    runtime::{
        migrations::{InitMigrationError, MigrationError},
        oneshot::Receiver,
        BlockchainData, CoreError, DispatcherSchema, ErrorMatch, MethodId, RuntimeIdentifier,
        SnapshotExt, WellKnownRuntime,
    },
};

const DELAY: Duration = Duration::from_millis(40);

#[derive(Default, Debug, Clone)]
struct MigrationRuntime {
    /// Flag to run good or erroneous migration script for `good-or-not-good` artifact.
    run_good_script: bool,
}

impl MigrationRuntime {
    fn with_script_flag(flag: bool) -> Self {
        Self {
            run_good_script: flag,
        }
    }
}

impl WellKnownRuntime for MigrationRuntime {
    const ID: u32 = 2;
}

impl Runtime for MigrationRuntime {
    fn deploy_artifact(&mut self, _artifact: ArtifactId, _deploy_spec: Vec<u8>) -> Receiver {
        Receiver::with_result(Ok(()))
    }

    // We use service freezing in some tests.
    fn is_supported(&self, feature: &RuntimeFeature) -> bool {
        match feature {
            RuntimeFeature::FreezingServices => true,
        }
    }

    fn is_artifact_deployed(&self, _id: &ArtifactId) -> bool {
        true
    }

    fn initiate_adding_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn initiate_resuming_service(
        &self,
        _context: ExecutionContext<'_>,
        _artifact: &ArtifactId,
        _parameters: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn update_service_status(&mut self, _snapshot: &dyn Snapshot, _state: &InstanceState) {}

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
                let version1 = Version::new(0, 2, 0);
                let version2 = Version::new(0, 3, 0);

                if *data_version < version1 {
                    end_version = version1;
                    complex_migration_part1
                } else if *data_version < version2 && new_artifact.version >= version2 {
                    end_version = version2;
                    complex_migration_part2
                } else {
                    return Ok(None);
                }
            }
            "not-good" => erroneous_migration,
            "bad" => panicking_migration,
            "with-state" => migration_modifying_state_hash,
            "none" => return Ok(None),
            "good-or-not-good" => {
                if self.run_good_script {
                    simple_delayed_migration
                } else {
                    erroneous_migration
                }
            }
            _ => return Err(InitMigrationError::NotSupported),
        };
        let script = MigrationScript::new(script, end_version);
        Ok(Some(script))
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        _method_id: MethodId,
        _arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn before_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
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
    assert!(ctx.data_version >= Version::new(0, 2, 0));
    assert!(ctx.data_version < Version::new(0, 3, 0));
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
        Self::with_db_and_flag(Arc::new(TemporaryDB::new()), false)
    }

    fn with_db_and_flag(db: Arc<TemporaryDB>, flag: bool) -> Self {
        let blockchain = Blockchain::new(
            db as Arc<dyn Database>,
            KeyPair::random(),
            ApiSender::closed(),
        );
        let blockchain = blockchain
            .into_mut_with_dummy_config()
            .with_runtime(MigrationRuntime::with_script_flag(flag))
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
            .with_runtime(MigrationRuntime::default())
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
        let block_params = BlockParams::new(ValidatorId(0), Height(100), &[]);
        let patch = self
            .blockchain
            .create_patch_inner(fork, &block_params, &[], &());
        self.blockchain.commit(patch, vec![]).unwrap();
        self.blockchain.as_ref().last_block()
    }

    fn deploy_artifact(&mut self, name: &str, version: Version) -> ArtifactId {
        let artifact = ArtifactId::from_raw_parts(MigrationRuntime::ID, name.into(), version);

        let fork = self.blockchain.fork();
        Dispatcher::commit_artifact(&fork, &artifact, vec![]);
        self.create_block(fork);
        artifact
    }

    fn initialize_service(&mut self, artifact: ArtifactId, name: &str) -> InstanceSpec {
        let service = InstanceSpec::from_raw_parts(self.next_service_id, name.to_owned(), artifact);
        self.next_service_id += 1;

        let mut fork = self.blockchain.fork();
        let mut should_rollback = false;
        let mut context = ExecutionContext::for_block_call(
            self.dispatcher(),
            &mut fork,
            &mut should_rollback,
            service.as_descriptor(),
        );
        context
            .initiate_adding_service(service.clone(), vec![])
            .expect("`initiate_adding_service` failed");
        assert!(!should_rollback);
        self.create_block(fork);
        service
    }

    fn stop_service(&mut self, spec: &InstanceSpec) {
        let fork = self.blockchain.fork();
        Dispatcher::initiate_stopping_service(&fork, spec.id).unwrap();
        self.create_block(fork);
    }

    fn freeze_service(&mut self, spec: &InstanceSpec) {
        let fork = self.blockchain.fork();
        self.dispatcher()
            .initiate_freezing_service(&fork, spec.id)
            .unwrap();
        self.create_block(fork);
    }
}

fn test_migration_workflow(freeze_service: bool) {
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
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::InvalidServiceTransition)
            .with_description_containing("Data migration cannot be initiated")
    );

    // Stop or freeze the service.
    if freeze_service {
        rig.freeze_service(&service);
    } else {
        rig.stop_service(&service);
    }

    // Now, the migration start should succeed.
    let fork = rig.blockchain.fork();
    let ty = rig
        .dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    assert_matches!(ty, MigrationType::Async);
    // Migration scripts should not start executing immediately, but only on block commit.
    assert!(!rig.migration_threads().contains_key(&service.name));
    // Check that the migration target cannot be unloaded.
    let err = Dispatcher::unload_artifact(&fork, &new_artifact).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUnloadArtifact)
            .with_description_containing("`100:good` references it as the data migration target")
    );
    rig.create_block(fork);

    // Check that the migration was initiated.
    assert!(rig.migration_threads().contains_key(&service.name));
    // Check that the old service data can be accessed.
    let snapshot = rig.blockchain.snapshot();
    assert!(snapshot.for_service(service.id).is_some());

    // Check that it is now impossible to unload either the old or the new artifact.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::unload_artifact(&fork, &old_artifact).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUnloadArtifact)
            .with_description_containing("`100:good` references it as the current artifact")
    );
    let err = Dispatcher::unload_artifact(&fork, &new_artifact).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUnloadArtifact)
            .with_description_containing("`100:good` references it as the data migration target")
    );

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
    assert!(!rig.migration_threads().contains_key(&service.name));

    // Create couple more blocks to check that the migration script is not launched again,
    // and the migration result is not overridden (these checks are `debug_assert`s
    // in the `Dispatcher` code).
    for _ in 0..3 {
        rig.create_block(rig.blockchain.fork());
    }
    assert!(!rig.migration_threads().contains_key(&service.name));
}

/// Tests basic workflow of migration initiation.
#[test]
fn migration_workflow() {
    test_migration_workflow(false);
}

#[test]
fn migration_workflow_with_frozen_service() {
    test_migration_workflow(true);
}

#[test]
fn migration_after_artifact_unloading() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact, "good");

    // Stop the service.
    rig.stop_service(&service);

    // Mark the new artifact for unload. This is valid because so far, no services are
    // associated with it.
    let fork = rig.blockchain.fork();
    Dispatcher::unload_artifact(&fork, &new_artifact).unwrap();
    // However, unloading means that we cannot initiate migration to the artifact.
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap_err();
    let expected_msg =
        "artifact `2:good:0.5.2` for data migration of service `100:good` is not active";
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::ArtifactNotDeployed)
            .with_description_containing(expected_msg)
    );
}

fn test_fast_forward_migration(freeze_service: bool) {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("none", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("none", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact.clone(), "service");
    if freeze_service {
        rig.freeze_service(&service);
    } else {
        rig.stop_service(&service);
    }

    let fork = rig.blockchain.fork();
    let ty = rig
        .dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &service.name)
        .unwrap();
    assert_matches!(ty, MigrationType::FastForward);
    rig.create_block(fork);

    // Service version should be updated when the block is merged.
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let state = schema.get_instance(service.id).unwrap();
    assert_eq!(state.status, Some(InstanceStatus::Stopped));
    assert_eq!(state.pending_status, None);
    assert_eq!(state.spec.artifact, new_artifact);
    assert_eq!(state.data_version, None);

    // Check that the old artifact can now be unloaded.
    let fork = rig.blockchain.fork();
    Dispatcher::unload_artifact(&fork, &old_artifact).unwrap();
    rig.create_block(fork);
    let snapshot = rig.blockchain.snapshot();
    assert!(DispatcherSchema::new(&snapshot)
        .get_artifact(&old_artifact)
        .is_none());
}

/// Tests fast-forwarding a migration.
#[test]
fn fast_forward_migration() {
    test_fast_forward_migration(false);
}

#[test]
fn fast_forward_migration_with_service_freezing() {
    test_fast_forward_migration(true);
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
        .initiate_migration(&fork, unrelated_artifact, &old_service.name)
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUpgradeService).with_any_description()
    );

    // Attempt to downgrade service.
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, old_artifact, &new_service.name)
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUpgradeService).with_any_description()
    );

    // Attempt to migrate to the same version.
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, new_artifact.clone(), &new_service.name)
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::CannotUpgradeService).with_any_description()
    );

    // Attempt to migrate unknown service.
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, new_artifact, "bogus-service")
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::IncorrectInstanceId)
            .with_description_containing("for non-existing service `bogus-service`")
    );

    // Attempt to migrate to unknown artifact.
    let unknown_artifact = ArtifactId::from_raw_parts(
        RuntimeIdentifier::Rust as _,
        "good".into(),
        Version::new(0, 6, 0),
    );
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, unknown_artifact.clone(), &old_service.name)
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::UnknownArtifactId).with_any_description()
    );

    // Mark the artifact as pending.
    Dispatcher::commit_artifact(&fork, &unknown_artifact, vec![]);
    let err = rig
        .dispatcher()
        .initiate_migration(&fork, unknown_artifact, &old_service.name)
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::ArtifactNotDeployed).with_any_description()
    );
}

/// Tests that an unfinished migration script is restarted on node restart.
#[test]
fn migration_is_resumed_after_node_restart() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact, "good");
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
    let service = rig.initialize_service(old_artifact, "good");
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
    let service = rig.initialize_service(old_artifact, "good");
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
    let service = rig.initialize_service(old_artifact, "service");
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
    let another_service = rig.initialize_service(old_artifact, "another-service");
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
        .initiate_migration(&fork, new_artifact, &another_service.name)
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
    let service = rig.initialize_service(old_artifact, "service");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    let state_hash = rig.create_block(fork).state_hash;

    for _ in 0..2 {
        // The sleeping interval is chosen to be larger than the interval of DB merges
        // in the migration script.
        thread::sleep(DELAY * 2 / 3);

        let fork = rig.blockchain.fork();
        // Check that we can access the old service data from outside.
        let blockchain_data = BlockchainData::new(&fork, "test");
        assert!(!blockchain_data
            .for_service(service.id)
            .unwrap()
            .get_proof_entry::<_, u32>("entry")
            .exists());
        // Check that the state during migration does not influence the default `state_hash`.
        let new_state_hash = rig.create_block(fork).state_hash;
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
    let service = rig.initialize_service(old_artifact, "good");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
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
    let service = rig.initialize_service(old_artifact, "good");

    // Non-existing service.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::rollback_migration(&fork, "bogus").unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::IncorrectInstanceId)
            .with_description_containing("Cannot rollback migration for unknown service `bogus`")
    );

    // Service is not stopped.
    let err = Dispatcher::rollback_migration(&fork, &service.name).unwrap_err();
    let no_migration_match = ErrorMatch::from_fail(&CoreError::NoMigration)
        .with_description_containing("it has no ongoing migration");
    assert_eq!(err, no_migration_match);

    rig.stop_service(&service);

    // Service is stopped, but there is no migration happening.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::rollback_migration(&fork, &service.name).unwrap_err();
    assert_eq!(err, no_migration_match);

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
    assert_eq!(err, no_migration_match);
}

/// Tests that migration rollback aborts locally executed migration script.
#[test]
fn migration_rollback_aborts_migration_script() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("with-state", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("with-state", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact, "good");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
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
    let service = rig.initialize_service(old_artifact, "good");
    rig.stop_service(&service);

    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
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
    let service = rig.initialize_service(old_artifact, "good");
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
    let expected_status = InstanceStatus::migrating(InstanceMigration::from_raw_parts(
        new_artifact,
        Version::new(0, 5, 0),
        Some(HashTag::empty_map_hash()),
    ));
    assert_eq!(state.status, Some(expected_status));
}

/// Tests checks performed by the dispatcher during migration commit.
#[test]
fn migration_commit_invariants() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact, "good");

    // Non-existing service.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::commit_migration(&fork, "bogus", Hash::zero()).unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&CoreError::IncorrectInstanceId)
            .with_description_containing("Cannot commit migration for unknown service `bogus`")
    );

    // Service is not stopped.
    let err = Dispatcher::commit_migration(&fork, &service.name, Hash::zero()).unwrap_err();
    let no_migration_match = ErrorMatch::from_fail(&CoreError::NoMigration)
        .with_description_containing("Cannot commit migration for service `100:good`");
    assert_eq!(err, no_migration_match);

    rig.stop_service(&service);

    // Service is stopped, but there is no migration happening.
    let fork = rig.blockchain.fork();
    let err = Dispatcher::commit_migration(&fork, &service.name, Hash::zero()).unwrap_err();
    assert_eq!(err, no_migration_match);

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
    assert_eq!(err, no_migration_match);
}

/// Tests that a migration commit after the migration script finished locally with an error
/// leads to node stopping.
fn test_migration_commit_with_local_error(
    rig: &mut Rig,
    local_result: LocalResult,
    artifact_name: &str,
) {
    let old_artifact = rig.deploy_artifact(artifact_name, "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact(artifact_name, "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact, "service");
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
    test_migration_commit_with_local_error(&mut Rig::new(), LocalResult::None, "not-good");
}

#[test]
#[should_panic(expected = "locally it has finished with an error: This migration is unsuccessful")]
fn migration_commit_with_local_error_in_memory() {
    test_migration_commit_with_local_error(&mut Rig::new(), LocalResult::InMemory, "not-good");
}

#[test]
#[should_panic(expected = "locally it has finished with an error: This migration is unsuccessful")]
fn migration_commit_with_local_error_saved() {
    test_migration_commit_with_local_error(&mut Rig::new(), LocalResult::Saved, "not-good");
}

#[test]
#[should_panic(expected = "locally it has finished with an error: This migration is unsuccessful")]
fn migration_commit_with_local_error_saved_and_node_restart() {
    test_migration_commit_with_local_error(
        &mut Rig::new(),
        LocalResult::SavedWithNodeRestart,
        "not-good",
    );
}

#[test]
fn test_migration_restart() {
    let artifact_name = "good-or-not-good";
    let service_name = "service";
    let db = Arc::new(TemporaryDB::new());

    // Running migration that should fail.
    std::panic::catch_unwind(|| {
        // Set script flag to fail migration.
        let mut rig = Rig::with_db_and_flag(Arc::clone(&db), false);
        test_migration_commit_with_local_error(&mut rig, LocalResult::Saved, artifact_name)
    })
    .expect_err("Node should panic on unsuccessful migration commit");

    // Check that we have failed result locally.
    let snapshot = db.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let res = schema
        .local_migration_result(service_name)
        .expect("Schema does not have local result");
    assert_eq!(res.0.unwrap_err(), "This migration is unsuccessful!");

    // Remove local migration result.
    let mut fork = db.fork();
    rollback_migration(&mut fork, service_name);
    remove_local_migration_result(&fork, service_name);
    db.merge_sync(fork.into_patch())
        .expect("Failed to merge patch after local migration result remove");

    // Check that local result is removed.
    let snapshot = db.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    assert!(schema.local_migration_result(service_name).is_none());

    // Set script flag to migrate successfully.
    let mut rig = Rig::with_db_and_flag(Arc::clone(&db), true);

    let fork = rig.blockchain.fork();
    Dispatcher::commit_migration(&fork, service_name, HashTag::empty_map_hash())
        .expect("Failed to commit migration");
    rig.create_block(fork);

    // Check that the migration script has finished.
    rig.assert_no_migration_threads();

    // Check that local migration result is erased.
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let res = schema.local_migration_result(service_name).unwrap();
    assert_eq!(res.0.unwrap(), HashTag::empty_map_hash());

    // Check current instance migration status.
    let state = schema.get_instance(100).unwrap();
    let artifact = ArtifactId::from_raw_parts(
        MigrationRuntime::ID,
        artifact_name.to_string(),
        "0.5.2".parse().unwrap(),
    );
    let expected_status = InstanceStatus::migrating(InstanceMigration::from_raw_parts(
        artifact,
        Version::new(0, 5, 0),
        Some(HashTag::empty_map_hash()),
    ));
    assert_eq!(state.status, Some(expected_status));
}

/// Tests that a migration commit after the migration script finished locally with another hash
/// leads to node stopping.
fn test_migration_commit_with_differing_hash(local_result: LocalResult) {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("good", "0.3.0".parse().unwrap());
    let new_artifact = rig.deploy_artifact("good", "0.5.2".parse().unwrap());
    let service = rig.initialize_service(old_artifact, "service");
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
    let service = rig.initialize_service(old_artifact, "test");
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
    let expected_status = InstanceStatus::migrating(InstanceMigration::from_raw_parts(
        new_artifact,
        Version::new(0, 5, 0),
        Some(migration_hash),
    ));
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
    let service = rig.initialize_service(old_artifact.clone(), "test");
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

    // The old artifact can now be unloaded, since it's no longer associated with the service.
    // In other words, the service cannot be started with the old artifact due to a different
    // data layout, so it can be removed from the blockchain.
    let fork = rig.blockchain.fork();
    Dispatcher::unload_artifact(&fork, &old_artifact).unwrap();

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

    // Check that the new artifact can be unloaded.
    let fork = rig.blockchain.fork();
    Dispatcher::unload_artifact(&fork, &new_artifact).unwrap();
    rig.create_block(fork);
}

#[test]
fn two_part_migration_with_intermediate_artifact() {
    let mut rig = Rig::new();
    let old_artifact = rig.deploy_artifact("complex", "0.1.1".parse().unwrap());
    let intermediate_artifact = rig.deploy_artifact("complex", "0.2.2".parse().unwrap());
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

    // Use a fast-forward migration to associate the service with an intermediate artifact.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, intermediate_artifact.clone(), &service.name)
        .unwrap();
    rig.create_block(fork);

    // Check service data and metadata.
    let snapshot = rig.blockchain.snapshot();
    assert_eq!(
        snapshot.get_proof_entry::<_, u32>("test.entry").get(),
        Some(1)
    );
    let schema = DispatcherSchema::new(&snapshot);
    let instance_state = schema.get_instance(service.id).unwrap();
    assert_eq!(instance_state.status, Some(InstanceStatus::Stopped));
    assert_eq!(instance_state.spec.artifact, intermediate_artifact);
    assert_eq!(instance_state.data_version, None);

    // Second part of migration. Since we've associated the service with a newer artifact,
    // the state will indicate that read endpoints may be retained for the service.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .initiate_migration(&fork, new_artifact, &service.name)
        .unwrap();
    rig.create_block(fork);

    thread::sleep(DELAY * 5);
    let migration_hash = rig.migration_hash(&[("test.entry", 2_u32.object_hash())]);
    let fork = rig.blockchain.fork();
    // Check that intermediate blockchain data can be accessed.
    let blockchain_data = BlockchainData::new(&fork, "other");
    let entry_value = blockchain_data
        .for_service(service.id)
        .unwrap()
        .get_proof_entry::<_, u32>("entry")
        .get();
    assert_eq!(entry_value, Some(1));
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
