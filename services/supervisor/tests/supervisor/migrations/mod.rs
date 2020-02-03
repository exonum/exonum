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

use exonum::{
    crypto::Hash,
    helpers::{Height, ValidatorId},
    merkledb::access::Prefixed,
    runtime::{
        migrations::MigrationStatus, versioning::Version, CoreError, ErrorMatch, ExecutionError,
        InstanceId, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_rust_runtime::{DefaultInstance, ServiceFactory};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};

use exonum_supervisor::{
    api::MigrationInfoQuery, AsyncEventState, ConfigPropose, MigrationError, MigrationRequest,
    MigrationResult, MigrationState, SchemaImpl, Supervisor, SupervisorInterface,
};

use std::{thread, time::Duration};

use crate::service_lifecycle::execute_transaction;

use migration_service::{
    FailingMigrationServiceV07, MigrationService, MigrationServiceV01_1, MigrationServiceV02,
    MigrationServiceV05, MigrationServiceV05_1,
};

mod migration_service;

/// Creates testkit with supervisor and several versions of migrating service.
///
/// One instance (with lowest version, "0.1.0") is started by default.
fn testkit_with_supervisor_and_service(validator_count: u16) -> TestKit {
    // Initialize builder.
    let builder = TestKitBuilder::validator().with_validators(validator_count);

    // Add supervisor.
    let builder = builder
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple());

    // Add MigrationService with running instance.
    let builder = builder.with_default_rust_service(MigrationService);

    // Add migrating artifact for version 0.1.1.
    let builder = builder
        .with_migrating_rust_service(MigrationServiceV01_1)
        .with_artifact(MigrationServiceV01_1.artifact_id());

    // Add migrating artifact for version 0.2.
    let builder = builder
        .with_migrating_rust_service(MigrationServiceV02)
        .with_artifact(MigrationServiceV02.artifact_id());

    // Add artifact for version 0.5.
    let builder = builder
        .with_migrating_rust_service(MigrationServiceV05)
        .with_artifact(MigrationServiceV05.artifact_id());

    // Add artifact for version 0.5.1.
    let builder = builder
        .with_migrating_rust_service(MigrationServiceV05_1)
        .with_artifact(MigrationServiceV05_1.artifact_id());

    builder.build()
}

/// Same as `testkit_with_supervisor_and_service`, but services do not support migrations.
fn testkit_with_supervisor_and_service_no_migrations(validator_count: u16) -> TestKit {
    // Initialize builder;
    let builder = TestKitBuilder::validator().with_validators(validator_count);

    // Add supervisor.
    let builder = builder
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple());

    // Add MigrationService with running instance.
    let builder = builder.with_default_rust_service(MigrationService);

    // Add migrating artifact for version 0.2.
    let builder = builder
        .with_rust_service(MigrationServiceV02)
        .with_artifact(MigrationServiceV02.artifact_id());

    // Add artifact for version 0.5.
    let builder = builder
        .with_rust_service(MigrationServiceV05)
        .with_artifact(MigrationServiceV05.artifact_id());

    builder.build()
}

/// Sends a `MigrationRequest` to supervisor through API.
fn request_migration(api: &TestKitApi, request: MigrationRequest) -> Hash {
    let hash: Hash = api
        .private(ApiKind::Service("supervisor"))
        .query(&request)
        .post("migrate")
        .unwrap();
    hash
}

/// Obtains a migration state through API.
fn migration_state(api: &TestKitApi, request: MigrationRequest) -> MigrationState {
    let query: MigrationInfoQuery = request.into();
    api.private(ApiKind::Service("supervisor"))
        .query(&query)
        .get("migration-status")
        .unwrap()
}

/// Stops service with the given ID.
fn stop_service(testkit: &mut TestKit, id: InstanceId) {
    let change = ConfigPropose::immediate(0).stop_service(id);
    let change = testkit
        .us()
        .service_keypair()
        .propose_config_change(SUPERVISOR_INSTANCE_ID, change);
    execute_transaction(testkit, change).expect("Stop service transaction should be processed");
}

fn obtain_reference_hash(testkit: &mut TestKit, request: &MigrationRequest) -> Hash {
    for _ in 0..5 {
        let snapshot = testkit.snapshot();
        let prefixed = Prefixed::new(Supervisor::NAME, &snapshot);
        let schema = SchemaImpl::new(prefixed);
        let state = schema.migration_state_unchecked(request);

        assert!(
            state.is_pending(),
            "State changed from pending while awaiting for expected hash: {:?}",
            state
        );

        let reference_hash = state.reference_state_hash();

        if let Some(reference_hash) = reference_hash {
            return *reference_hash;
        } else {
            // Migration is executed in the separate thread, so sleep a bit.
            thread::sleep(Duration::from_millis(50));
            // Then create a new block.
            testkit.create_block();
        }
    }
    panic!("Node didn't calculate the expected hash")
}

/// Waits for `MigrationStatus` to change from pending and returns a new status.
/// Panics if reaches deadline height and state is still `Pending`.
fn wait_while_pending(
    testkit: &mut TestKit,
    deadline_height: Height,
    request: MigrationRequest,
) -> MigrationState {
    let api = testkit.api();
    while testkit.height() <= deadline_height.next() {
        testkit.create_block();
        let migration_state = migration_state(&api, request.clone());

        match migration_state.inner {
            AsyncEventState::Pending => {
                // Not ready yet.
            }
            _ => {
                return migration_state;
            }
        }
    }

    panic!("Migration is pending after reaching deadline height");
}

/// Waits for the migration associated with provides request will result
/// in a success. Panics otherwise.
fn wait_for_migration_success(
    testkit: &mut TestKit,
    deadline_height: Height,
    request: MigrationRequest,
    version: Version,
) {
    let state = wait_while_pending(testkit, deadline_height, request);
    if let AsyncEventState::Succeed = state.inner {
        assert_eq!(state.version, version);
    } else {
        panic!("Migration failed: {:?}", state);
    }
}

/// Waits for the migration associated with provides request will result
/// in a failure. Panics otherwise.
fn wait_for_migration_fail(
    testkit: &mut TestKit,
    deadline_height: Height,
    request: MigrationRequest,
) -> ExecutionError {
    let state = wait_while_pending(testkit, deadline_height, request);
    if let AsyncEventState::Failed { error, .. } = state.inner {
        error
    } else {
        panic!("Migration not failed, but was expected to: {:?}", state);
    }
}

/// Waits for the migration associated with provides request will result
/// in a timeout. Panics otherwise.
fn wait_for_migration_timeout(
    testkit: &mut TestKit,
    deadline_height: Height,
    request: MigrationRequest,
) {
    let state = wait_while_pending(testkit, deadline_height, request);
    if let AsyncEventState::Timeout = state.inner {
        // That's expected
    } else {
        panic!("Migration not failed failed due to timeout: {:?}", state);
    }
}

/// Creates a migration request and checks that transaction with this request
/// is executed successfully.
fn send_migration_request(testkit: &mut TestKit, request: MigrationRequest) {
    let api = testkit.api();
    let tx_hash = request_migration(&api, request);
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");
}

const DEADLINE_HEIGHT: Height = Height(10);

/// Basic test scenario for a simple migration workflow.
///
/// Here we perform a migration with one migration script and one validator in
/// the network.
///
/// Expected behavior is that migration is completed successfully and schema
/// is updated to the next version of data.
#[test]
fn migration() {
    let mut testkit = testkit_with_supervisor_and_service(1);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigrationServiceV02.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v02::verify_schema(prefixed);
}

/// This test applies two migrations to one service, one after another.
#[test]
fn migration_two_scripts_sequential() {
    let mut testkit = testkit_with_supervisor_and_service(1);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration to 0.2.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigrationServiceV02.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v02::verify_schema(prefixed);

    // Request migration to 0.5.
    let deadline_height = Height(DEADLINE_HEIGHT.0 * 2);
    let request = MigrationRequest {
        new_artifact: MigrationServiceV05.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 5, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v05::verify_schema(prefixed);
}

/// Test for processing a failure during migration.
///
/// Here we perform a migration with one migration script which always fails.
///
/// Expected behavior is that migration is failed and no changes are applied to
/// data.
#[test]
fn migration_fail() {
    let mut testkit = {
        // Initialize builder;
        let builder = TestKitBuilder::validator();

        // Add supervisor.
        let builder = builder
            .with_rust_service(Supervisor)
            .with_artifact(Supervisor.artifact_id())
            .with_instance(Supervisor::simple());

        // Add MigrationService with running instance.
        let builder = builder
            .with_rust_service(MigrationServiceV05)
            .with_artifact(MigrationServiceV05.artifact_id())
            .with_instance(MigrationServiceV05.artifact_id().into_default_instance(
                MigrationService::INSTANCE_ID,
                MigrationService::INSTANCE_NAME,
            ));

        // Add migrating artifact for version 0.7.
        let builder = builder
            .with_migrating_rust_service(FailingMigrationServiceV07)
            .with_artifact(FailingMigrationServiceV07.artifact_id());

        builder.build()
    };

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: FailingMigrationServiceV07.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    let error = wait_for_migration_fail(&mut testkit, deadline_height, request);

    assert_eq!(
        error,
        ErrorMatch::from_fail(&MigrationError::MigrationFailed)
            .with_description_containing("This migration always fails")
    );
}

/// This test checks that migration that contains two migration scripts completes
/// successfully in two steps.
#[test]
fn complex_migration() {
    let mut testkit = testkit_with_supervisor_and_service(1);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration to 0.5.
    // This migration will require two migration requests.
    let deadline_height = DEADLINE_HEIGHT;
    let mut request = MigrationRequest {
        new_artifact: MigrationServiceV05.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    // After the first migration step, version should be "0.2".
    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request.clone(),
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v02::verify_schema(prefixed);

    // Request the same migration.
    let new_deadline_height = Height(DEADLINE_HEIGHT.0 * 2);
    request.deadline_height = new_deadline_height;

    send_migration_request(&mut testkit, request.clone());

    // Now we finally should have version "0.5".
    wait_for_migration_success(
        &mut testkit,
        new_deadline_height,
        request,
        Version::new(0, 5, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v05::verify_schema(prefixed);
}

/// This test checks that attempt to request a migration for service that doesn't support
/// migrations results in a migration failure.
#[test]
fn no_migration_support() {
    let mut testkit = testkit_with_supervisor_and_service_no_migrations(1);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigrationServiceV05.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    // Despite the fact that migration should fail, the transaction with request
    // should be executed successfully.
    send_migration_request(&mut testkit, request.clone());

    // Migration should not start and fail on the **next height**,
    // so we use it as a strict deadline.
    let next_height = testkit.height().next();
    let error = wait_for_migration_fail(&mut testkit, next_height, request);

    assert_eq!(
        error,
        ErrorMatch::from_fail(&CoreError::NoMigration).with_any_description()
    );
}

/// Test for a migration workflow with multiple validators.
///
/// After execution of migration locally, testkit receives transactions with
/// reports about successful migration from other nodes.
///
/// Expected behavior is that migration is completed successfully and schema
/// is updated to the next version of data.
#[test]
fn migration_consensus() {
    let validators_amount = 5;
    let mut testkit = testkit_with_supervisor_and_service(validators_amount);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigrationServiceV02.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    // Obtain the expected migration hash and send confirmations from other nodes.
    let reference_hash = obtain_reference_hash(&mut testkit, &request);

    let migration_status = MigrationStatus(Ok(reference_hash));
    let migration_result = MigrationResult {
        request: request.clone(),
        status: migration_status,
    };

    // Build confirmation transactions
    let confirmations: Vec<_> = (1..validators_amount)
        .map(|i| {
            let keypair = testkit.validator(ValidatorId(i)).service_keypair();
            keypair.report_migration_result(SUPERVISOR_INSTANCE_ID, migration_result.clone())
        })
        .collect();

    // Check that before obtaining confirmations, migration state is pending.
    let api = testkit.api();
    let migration_state = migration_state(&api, request.clone());
    assert!(migration_state.is_pending());

    testkit.create_block_with_transactions(confirmations);

    // Now wait for migration success.
    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v02::verify_schema(prefixed);
}

/// Test for a migration workflow with multiple validators.
///
/// This test is similar to `migration_consensus`, but not all validators
/// send their confirmation.
///
/// Expected behavior is that migration is failed due to timeout.
#[test]
fn migration_no_consensus() {
    let validators_amount = 5;
    let mut testkit = testkit_with_supervisor_and_service(validators_amount);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigrationServiceV02.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    // Obtain the expected migration hash and send confirmations from other nodes.
    let reference_hash = obtain_reference_hash(&mut testkit, &request);

    let migration_status = MigrationStatus(Ok(reference_hash));
    let migration_result = MigrationResult {
        request: request.clone(),
        status: migration_status,
    };

    // Build confirmation transactions for every validator except one.
    let confirmations: Vec<_> = (1..(validators_amount - 1))
        .map(|i| {
            let keypair = testkit.validator(ValidatorId(i)).service_keypair();
            keypair.report_migration_result(SUPERVISOR_INSTANCE_ID, migration_result.clone())
        })
        .collect();

    // Check that before obtaining confirmations, migration state is pending.
    let api = testkit.api();
    let migration_state = migration_state(&api, request.clone());
    assert!(migration_state.is_pending());

    testkit.create_block_with_transactions(confirmations);

    // Now wait for migration timeout.
    wait_for_migration_timeout(&mut testkit, deadline_height, request);

    // After that check that schema did not change.
    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v01::verify_schema(prefixed);
}

/// Test for a migration workflow with multiple validators.
///
/// This test checks that if node obtains different state hashes,
/// migration fails and no changes are performed to schema.
///
/// Expected behavior is that migration is failed.
#[test]
fn migration_hash_divergence() {
    let validators_amount = 5;
    let mut testkit = testkit_with_supervisor_and_service(validators_amount);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigrationServiceV02.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    // Obtain the expected migration hash and send confirmations from other nodes.
    let reference_hash = obtain_reference_hash(&mut testkit, &request);

    let migration_status = MigrationStatus(Ok(reference_hash));
    let migration_result = MigrationResult {
        request: request.clone(),
        status: migration_status,
    };

    // Build confirmation transactions for every validator except one.
    let mut confirmations: Vec<_> = (1..(validators_amount - 1))
        .map(|i| {
            let keypair = testkit.validator(ValidatorId(i)).service_keypair();
            keypair.report_migration_result(SUPERVISOR_INSTANCE_ID, migration_result.clone())
        })
        .collect();

    // For a missing validator, create an incorrect hash report.
    let wrong_status = MigrationStatus(Ok(Hash::zero()));
    let wrong_result = MigrationResult {
        request: request.clone(),
        status: wrong_status,
    };

    let wrong_confirmation = {
        let last_validator_id = validators_amount - 1;
        let keypair = testkit
            .validator(ValidatorId(last_validator_id))
            .service_keypair();
        keypair.report_migration_result(SUPERVISOR_INSTANCE_ID, wrong_result)
    };

    confirmations.push(wrong_confirmation);

    // Check that before obtaining confirmations, migration state is pending.
    let api = testkit.api();
    let migration_state = migration_state(&api, request.clone());
    assert!(migration_state.is_pending());

    testkit.create_block_with_transactions(confirmations);

    // Now wait for migration timeout.
    let error = wait_for_migration_fail(&mut testkit, deadline_height, request);

    assert_eq!(
        error,
        ErrorMatch::from_fail(&MigrationError::StateHashDivergence).with_any_description()
    );

    // After that check that schema did not change.
    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v01::verify_schema(prefixed);
}

/// Test for a fast-forward migration (0.1.0 - 0.1.1)
#[test]
fn fast_forward_migration() {
    let mut testkit = testkit_with_supervisor_and_service(1);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigrationServiceV01_1.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 1, 1),
    );
}

/// This test checks mixed migration scenario: two data migrations and one fast-forward.
#[test]
fn mixed_migration() {
    let mut testkit = testkit_with_supervisor_and_service(1);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigrationService::INSTANCE_ID);

    // Request migration to 0.5.1.
    // This migration will require three migration requests.
    let deadline_height = DEADLINE_HEIGHT;
    let mut request = MigrationRequest {
        new_artifact: MigrationServiceV05_1.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    send_migration_request(&mut testkit, request.clone());

    // After the first migration step, version should be "0.2".
    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request.clone(),
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v02::verify_schema(prefixed);

    // Request the same migration.
    let new_deadline_height = Height(DEADLINE_HEIGHT.0 * 2);
    request.deadline_height = new_deadline_height;

    send_migration_request(&mut testkit, request.clone());

    // Now we should have version "0.5".
    wait_for_migration_success(
        &mut testkit,
        new_deadline_height,
        request.clone(),
        Version::new(0, 5, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    migration_service::v05::verify_schema(prefixed);

    // Request the same migration for the third time.
    let even_newer_deadline_height = Height(DEADLINE_HEIGHT.0 * 3);
    request.deadline_height = even_newer_deadline_height;

    send_migration_request(&mut testkit, request.clone());

    // Now we finally should have version "0.5.1".
    wait_for_migration_success(
        &mut testkit,
        even_newer_deadline_height,
        request,
        Version::new(0, 5, 1),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, snapshot.as_ref());

    // Data should not change.
    migration_service::v05::verify_schema(prefixed);
}
