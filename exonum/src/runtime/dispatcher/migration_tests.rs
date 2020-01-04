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
use exonum_merkledb::{HashTag, TemporaryDB};
use futures::IntoFuture;

use std::time::Duration;

use super::*;
use crate::runtime::RuntimeIdentifier;
use crate::{
    blockchain::{Block, BlockchainMut},
    helpers::ValidatorId,
    node::ApiSender,
    runtime::migrations::DataMigrationError,
    runtime::{CallInfo, DispatcherError, DispatcherSchema, ErrorMatch, WellKnownRuntime},
};

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

    fn update_service_status(
        &mut self,
        _snapshot: &dyn Snapshot,
        _spec: &InstanceSpec,
        _status: InstanceStatus,
    ) {
    }

    fn migrate(
        &self,
        _new_artifact: &ArtifactId,
        old_service: &InstanceSpec,
    ) -> Result<Vec<MigrationScript>, DataMigrationError> {
        if old_service.artifact.name == "good" {
            let script = |_: &mut MigrationContext| {
                thread::sleep(Duration::from_millis(200));
            };
            let script = MigrationScript::new(script, Version::new(0, 5, 0));
            Ok(vec![script])
        } else {
            Err(DataMigrationError::NotSupported)
        }
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

    fn dispatcher(&mut self) -> &mut Dispatcher {
        self.blockchain.dispatcher()
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
        .start_migration(&fork, new_artifact.clone(), service.id.into())
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::ServiceNotStopped)
    );

    // Stop the service.
    rig.stop_service(&service);

    // Now, the migration start should succeed.
    let fork = rig.blockchain.fork();
    rig.dispatcher()
        .start_migration(&fork, new_artifact, service.id.into())
        .unwrap();
    // Migration scripts should not start executing immediately, but only on block commit.
    assert!(!rig
        .dispatcher()
        .migrations
        .threads
        .contains_key(&service.name));
    rig.create_block(fork);

    // Check that the migration was initiated.
    let migration = &rig.dispatcher().migrations.threads[&service.name];
    assert_eq!(migration.end_version, Version::new(0, 5, 0));

    // Create several more blocks before the migration is complete and check that
    // we don't spawn multiple migration scripts at once (this check is performed in `Migrations`).
    for _ in 0..3 {
        rig.create_block(rig.blockchain.fork());
    }

    // Wait until the migration script is completed and check that its result is recorded.
    thread::sleep(Duration::from_millis(500));

    rig.create_block(rig.blockchain.fork());
    let snapshot = rig.blockchain.snapshot();
    let schema = DispatcherSchema::new(&snapshot);
    let migration = schema.completed_migration(&service.name).unwrap();
    assert_eq!(migration.instance, service);
    assert_eq!(migration.end_version, Version::new(0, 5, 0));
    assert_eq!(migration.result, Ok(HashTag::empty_map_hash()));
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

/// Tests checks performed by the dispatcher.
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
        .start_migration(&fork, unrelated_artifact.clone(), old_service.id.into())
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::CannotUpgradeService)
    );

    // Attempt to downgrade service.
    let err = rig
        .dispatcher()
        .start_migration(&fork, old_artifact, new_service.id.into())
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::CannotUpgradeService)
    );

    // Attempt to migrate to the same version.
    let err = rig
        .dispatcher()
        .start_migration(&fork, new_artifact.clone(), new_service.id.into())
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::CannotUpgradeService)
    );

    // Attempt to migrate unknown service.
    let err = rig
        .dispatcher()
        .start_migration(&fork, new_artifact, "bogus-service".into())
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::IncorrectInstanceId)
    );

    // Attempt to migrate to unknown artifact.
    let unknown_artifact = ArtifactId {
        runtime_id: RuntimeIdentifier::Rust as _,
        name: "good".to_owned(),
        version: Version::new(0, 6, 0),
    };
    let err = rig
        .dispatcher()
        .start_migration(&fork, unknown_artifact.clone(), old_service.id.into())
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::UnknownArtifactId)
    );

    // Mark the artifact as pending.
    Dispatcher::commit_artifact(&fork, unknown_artifact.clone(), vec![]);
    let err = rig
        .dispatcher()
        .start_migration(&fork, unknown_artifact, old_service.id.into())
        .unwrap_err();
    assert_eq!(
        err,
        ErrorMatch::from_fail(&DispatcherError::ArtifactNotDeployed)
    );
}

#[test]
fn migration_is_resumed_after_node_restart() {
    unimplemented!()
}

#[test]
fn migration_with_panic() {
    unimplemented!()
}

#[test]
fn concurrent_migrations() {
    unimplemented!()
}

#[test]
fn migration_influencing_state_hash() {
    unimplemented!()
}
