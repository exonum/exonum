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

//! Tests for the phases of the service life cycle, including starting, freezing and stopping
//! service instances.

use exonum::{
    helpers::Height,
    merkledb::Snapshot,
    messages::{AnyTx, Verified},
    runtime::{
        migrations::{InitMigrationError, MigrationScript},
        oneshot::Receiver,
        versioning::Version,
        ArtifactId, ErrorMatch, ExecutionError, InstanceState, InstanceStatus, Mailbox, Runtime,
        SnapshotExt, WellKnownRuntime, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_rust_runtime::{
    spec::{ForeignSpec, Spec},
    DefaultInstance, ExecutionContext, ServiceFactory,
};
use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};

use crate::inc::IncService;
use exonum_supervisor::{
    ArtifactError, ConfigPropose, ConfigurationError, Supervisor, SupervisorInterface,
};

#[derive(Debug, Clone, Copy)]
struct RuntimeWithoutFreeze;

impl RuntimeWithoutFreeze {
    fn artifact() -> ArtifactId {
        ArtifactId::from_raw_parts(Self::ID, "some-service".to_owned(), Version::new(1, 0, 0))
    }
}

impl Runtime for RuntimeWithoutFreeze {
    fn deploy_artifact(&mut self, _artifact: ArtifactId, _deploy_spec: Vec<u8>) -> Receiver {
        Receiver::with_result(Ok(()))
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
        unimplemented!("Outside the test scope")
    }

    fn update_service_status(&mut self, _snapshot: &dyn Snapshot, state: &InstanceState) {
        assert_ne!(state.status, Some(InstanceStatus::Frozen));
    }

    fn migrate(
        &self,
        _new_artifact: &ArtifactId,
        _data_version: &Version,
    ) -> Result<Option<MigrationScript>, InitMigrationError> {
        Err(InitMigrationError::NotSupported)
    }

    fn execute(
        &self,
        _context: ExecutionContext<'_>,
        _method_id: u32,
        _arguments: &[u8],
    ) -> Result<(), ExecutionError> {
        unimplemented!("Outside the test scope")
    }

    fn before_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {}
}

impl WellKnownRuntime for RuntimeWithoutFreeze {
    const ID: u32 = 5;
}

/// Creates block with the specified transaction and returns its execution result.
pub fn execute_transaction(
    testkit: &mut TestKit,
    tx: Verified<AnyTx>,
) -> Result<(), ExecutionError> {
    testkit.create_block_with_transaction(tx).transactions[0]
        .status()
        .map_err(Clone::clone)
}

/// Checks that the `inc` service API is available.
async fn is_inc_service_api_available(testkit: &mut TestKit) -> bool {
    testkit
        .api()
        .public(ApiKind::Service(IncService::INSTANCE_NAME))
        .get::<()>("v1/ping")
        .await
        .is_ok()
}

fn create_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with(Supervisor::simple())
        .with(Spec::new(IncService))
        .build()
}

fn create_testkit_with_additional_runtime() -> TestKit {
    let artifact = RuntimeWithoutFreeze::artifact();
    TestKitBuilder::validator()
        .with_additional_runtime(RuntimeWithoutFreeze)
        .with(ForeignSpec::new(artifact).with_instance(100, "test", ()))
        .with(Supervisor::simple())
        .build()
}

/// Starts service instance and gets its ID
fn start_inc_service(testkit: &mut TestKit) -> InstanceState {
    // Start `inc` service instance
    let change = ConfigPropose::immediate(0).start_service(
        IncService.artifact_id(),
        IncService::INSTANCE_NAME,
        Vec::default(),
    );
    let keypair = testkit.us().service_keypair();
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    execute_transaction(testkit, change).expect("Start service transaction should be processed");

    // Get started service instance ID.
    testkit
        .snapshot()
        .for_dispatcher()
        .get_instance(IncService::INSTANCE_NAME)
        .unwrap()
}

#[tokio::test]
async fn start_stop_inc_service() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();
    let instance_id = start_inc_service(&mut testkit).spec.id;
    assert!(
        is_inc_service_api_available(&mut testkit).await,
        "Inc service API should be available after starting."
    );

    // Stop service instance.
    let change = ConfigPropose::immediate(1).stop_service(instance_id);
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    execute_transaction(&mut testkit, change)
        .expect("Stop service transaction should be processed");
    assert!(
        !is_inc_service_api_available(&mut testkit).await,
        "Inc service API should not be available after stopping."
    );

    // Check that we cannot freeze service now.
    let change = ConfigPropose::immediate(2).freeze_service(instance_id);
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let err = execute_transaction(&mut testkit, change)
        .expect_err("Freeze service transaction should not be processed");
    let expected_err = ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
        .with_description_containing(
            "Discarded an attempt to freeze service `inc` with inappropriate status (stopped)",
        );
    assert_eq!(err, expected_err);
}

#[tokio::test]
async fn start_freeze_and_stop_inc_service() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();
    let instance_id = start_inc_service(&mut testkit).spec.id;

    // Freeze service instance.
    let change = ConfigPropose::immediate(1).freeze_service(instance_id);
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    execute_transaction(&mut testkit, change)
        .expect("Freeze service transaction should be processed");
    assert!(
        is_inc_service_api_available(&mut testkit).await,
        "Inc service API should be available after freezing."
    );

    // Stop the same service instance.
    let change = ConfigPropose::immediate(2).stop_service(instance_id);
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    execute_transaction(&mut testkit, change)
        .expect("Stop service transaction should be processed");
    assert!(
        !is_inc_service_api_available(&mut testkit).await,
        "Inc service API should not be available after stopping."
    );
}

#[test]
fn stop_non_existent_service() {
    let mut testkit = create_testkit();

    let instance_id = 2;
    let change = ConfigPropose::immediate(1).stop_service(instance_id);
    let keypair = testkit.us().service_keypair();
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let actual_err =
        execute_transaction(&mut testkit, change).expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing("Instance with ID 2 is absent from the blockchain")
    )
}

#[test]
fn unload_unused_artifact() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let artifact = IncService.artifact_id();
    let change = ConfigPropose::immediate(0).unload_artifact(artifact.clone());
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    execute_transaction(&mut testkit, change).expect("Cannot unload artifact");

    let snapshot = testkit.snapshot();
    assert!(snapshot.for_dispatcher().get_artifact(&artifact).is_none());

    // Check that an attempt to start an `IncService` fails now.
    let change = ConfigPropose::immediate(1).start_service(artifact.clone(), "test", ());
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let err = execute_transaction(&mut testkit, change).unwrap_err();
    let expected_msg = "Discarded start of service `test` from the unknown artifact";
    assert_eq!(
        err,
        ErrorMatch::from_fail(&ArtifactError::UnknownArtifact)
            .with_description_containing(expected_msg)
    );

    // So does unloading the artifact again.
    let change = ConfigPropose::immediate(1).unload_artifact(artifact);
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let err = execute_transaction(&mut testkit, change).unwrap_err();
    let expected_msg = "Requested to unload artifact `0:inc:1.0.0`, which is not deployed";
    assert_eq!(
        err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .with_description_containing(expected_msg)
    );
}

#[test]
fn unloading_used_artifact() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();
    start_inc_service(&mut testkit);

    let change = ConfigPropose::immediate(1).unload_artifact(IncService.artifact_id());
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let err = execute_transaction(&mut testkit, change).unwrap_err();
    let expected_msg = "Cannot unload artifact `0:inc:1.0.0`: service `1:inc` references it";
    assert_eq!(
        err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .with_description_containing(expected_msg)
    );
}

#[test]
fn unloading_artifact_with_concurrent_service_start() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();
    let artifact = IncService.artifact_id();
    let change = ConfigPropose::immediate(0)
        .start_service(artifact.clone(), "inc", ())
        .unload_artifact(artifact.clone());
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);

    let err = execute_transaction(&mut testkit, change).expect_err("Config should fail");
    let expected_msg = "starts a service from artifact `0:inc:1.0.0` and unloads it";
    assert_eq!(
        err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .with_description_containing(expected_msg)
    );

    // Switch two actions in the proposal around; it should still fail.
    let change = ConfigPropose::immediate(0)
        .unload_artifact(artifact.clone())
        .start_service(artifact, "inc", ());
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);

    let err = execute_transaction(&mut testkit, change).expect_err("Config should fail");
    assert_eq!(
        err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .with_description_containing(expected_msg)
    );
}

#[test]
fn duplicate_stop_service_request() {
    let mut testkit = create_testkit();
    let instance_id = start_inc_service(&mut testkit).spec.id;

    // An attempt to stop service twice.
    let change = ConfigPropose::immediate(1)
        .stop_service(instance_id)
        .stop_service(instance_id);
    let keypair = testkit.us().service_keypair();
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let actual_err =
        execute_transaction(&mut testkit, change).expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing("Discarded several actions concerning service with ID 1")
    );
}

#[test]
fn duplicate_artifact_unload_request() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let artifact = IncService.artifact_id();
    let change = ConfigPropose::immediate(0)
        .unload_artifact(artifact.clone())
        .unload_artifact(artifact);
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let err = execute_transaction(&mut testkit, change).expect_err("Change should fail");

    assert_eq!(
        err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .with_description_containing("Discarded multiple unloads of artifact `0:inc:1.0.0`")
    );
}

#[test]
fn stop_already_stopped_service() {
    let mut testkit = create_testkit();

    let instance_id = start_inc_service(&mut testkit).spec.id;
    let change = ConfigPropose::immediate(1).stop_service(instance_id);
    let keypair = testkit.us().service_keypair();
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    // Stop service instance.
    execute_transaction(&mut testkit, change).expect("Transaction should be processed");

    // Second attempt to stop service instance.
    let other_change = ConfigPropose::immediate(2).stop_service(instance_id);
    let other_change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, other_change);
    let actual_err = execute_transaction(&mut testkit, other_change)
        .expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing(
                "Discarded an attempt to stop service `inc` with inappropriate status (stopped)"
            )
    )
}

#[tokio::test]
async fn resume_stopped_service() {
    let mut testkit = create_testkit();

    // Stop service instance.
    let instance = start_inc_service(&mut testkit);
    let change = ConfigPropose::immediate(1).stop_service(instance.spec.id);
    let keypair = testkit.us().service_keypair();
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    execute_transaction(&mut testkit, change).expect("Transaction should be processed");

    // Resume service instance.
    let change = ConfigPropose::immediate(2).resume_service(instance.spec.id, vec![]);
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    execute_transaction(&mut testkit, change).expect("Transaction should be processed");

    // Check resumed service API.
    assert!(is_inc_service_api_available(&mut testkit).await);
}

#[test]
fn resume_active_service() {
    let mut testkit = create_testkit();
    let instance = start_inc_service(&mut testkit);

    // Resume service instance.
    let change = ConfigPropose::immediate(1).resume_service(instance.spec.id, vec![]);
    let keypair = testkit.us().service_keypair();
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let actual_err =
        execute_transaction(&mut testkit, change).expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing(
                "Discarded an attempt to resume service `inc` with inappropriate status (active)"
            )
    )
}

#[test]
fn multiple_stop_resume_requests() {
    let mut testkit = create_testkit();

    let spec = start_inc_service(&mut testkit).spec;

    // Config proposal with two requests for single service instance.
    let change = ConfigPropose::immediate(2)
        .stop_service(spec.id)
        .resume_service(spec.id, ());
    let keypair = testkit.us().service_keypair();
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let actual_err =
        execute_transaction(&mut testkit, change).expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing("Discarded several actions concerning service with ID 1")
    )
}

#[test]
fn freeze_without_runtime_support() {
    let mut testkit = create_testkit_with_additional_runtime();
    let change = ConfigPropose::new(0, Height(5)).freeze_service(100);
    let keypair = testkit.us().service_keypair();
    let change = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    let actual_err =
        execute_transaction(&mut testkit, change).expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .with_description_containing("Cannot freeze service `100:test`")
    );
}
