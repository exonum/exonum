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
        migrations::MigrationStatus, CoreError, ErrorMatch, ExecutionError, InstanceId, Version,
        SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_rust_runtime::{DefaultInstance, ServiceFactory};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};

use exonum_supervisor::{
    AsyncEventState, ConfigPropose, MigrationError, MigrationInfoQuery, MigrationRequest,
    MigrationResult, MigrationStateResponse, SchemaImpl, Supervisor, SupervisorInterface,
};

use std::{thread, time::Duration};

use crate::service_lifecycle::execute_transaction;

use migration_service::{
    FailingMigrationServiceV07, MigrationService, MigrationServiceV01_1, MigrationServiceV02,
    MigrationServiceV05, MigrationServiceV05_1,
};

mod migration_service;

/// Creates testkit with supervisor and three versions of migrating service.
///
/// One instance (with lowest version, "0.1.0") is started by default.
fn testkit_with_supervisor_and_service(validator_count: u16) -> TestKit {
    // Initialize builder;
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

    builder.create()
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

    builder.create()
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
fn migration_state(api: &TestKitApi, request: MigrationRequest) -> MigrationStateResponse {
    let query: MigrationInfoQuery = request.into();
    api.private(ApiKind::Service("supervisor"))
        .query(&query)
        .get("migration-status")
        .unwrap()
}

/// Stops service with the given ID.
fn stop_service(testkit: &mut TestKit, id: InstanceId) {
    let keypair = testkit.us().service_keypair();
    execute_transaction(
        testkit,
        ConfigPropose::immediate(0)
            .stop_service(id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect("Stop service transaction should be processed");
}

fn obtain_expected_hash(testkit: &mut TestKit, request: &MigrationRequest) -> Hash {
    for _ in 0..5 {
        let snapshot = testkit.snapshot();
        let prefixed = Prefixed::new(Supervisor::NAME, &snapshot);
        let schema = SchemaImpl::new(prefixed);
        let state = schema
            .migration_states
            .get(request)
            .expect("Migration state is not stored");

        assert!(
            state.state.is_pending(),
            "State changed from pending while awaiting for expected hash: {:?}",
            state
        );

        let expected_hash = state.expected_state_hash();

        if let Some(expected_hash) = expected_hash {
            return *expected_hash;
        } else {
            // Migration is executed in the separate thread, so sleep a bit.
            thread::sleep(Duration::from_millis(50));
            // Then create a new block.
            testkit.create_block();
        }
    }
    panic!("Node didn't calculate the expected hash")
}

fn wait_for_migration_success(
    testkit: &mut TestKit,
    deadline_height: Height,
    request: MigrationRequest,
    version: Version,
) {
    let mut success = false;
    while testkit.height() <= deadline_height.next() {
        testkit.create_block();
        let api = testkit.api();
        let migration_state = migration_state(&api, request.clone())
            .state
            .expect("State for requested migration is not stored");

        match migration_state.state {
            AsyncEventState::Pending => {
                // Not ready yet.
            }
            AsyncEventState::Succeed => {
                // Migration completed.
                assert_eq!(migration_state.version, version);
                success = true;
                break;
            }
            other => {
                panic!("Migration failed: {:?}", other);
            }
        }
    }

    assert!(success, "Migration did not end");

    // Migration is flushed at the next block after its success.
    testkit.create_block();
}

fn wait_for_migration_fail(
    testkit: &mut TestKit,
    deadline_height: Height,
    request: MigrationRequest,
) -> ExecutionError {
    while testkit.height() <= deadline_height.next() {
        testkit.create_block();
        let api = testkit.api();
        let migration_state = migration_state(&api, request.clone())
            .state
            .expect("State for requested migration is not stored");

        match migration_state.state {
            AsyncEventState::Pending => {
                // Not ready yet.
            }
            AsyncEventState::Succeed => panic!("Migration succeed, but was expected to fail"),
            AsyncEventState::Failed { error, .. } => {
                return error;
            }
            AsyncEventState::Timeout => {
                panic!("Migration was killed due to timeout, but was expected to fail explicitly")
            }
        }
    }

    panic!("Migration is pending after reaching deadline height");
}

fn wait_for_migration_timeout(
    testkit: &mut TestKit,
    deadline_height: Height,
    request: MigrationRequest,
) {
    while testkit.height() <= deadline_height.next() {
        testkit.create_block();
        let api = testkit.api();
        let migration_state = migration_state(&api, request.clone())
            .state
            .expect("State for requested migration is not stored");

        match migration_state.state {
            AsyncEventState::Pending => {
                // Not ready yet.
            }
            AsyncEventState::Timeout => {
                return;
            }
            other => panic!(
                "Migration ended and did not reach timeout: end state {:?}",
                other
            ),
        }
    }

    panic!("Migration is pending after reaching deadline height");
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

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

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

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

    migration_service::v02::verify_schema(prefixed);

    // Request migration to 0.5.
    let deadline_height = Height(DEADLINE_HEIGHT.0 * 2);
    let request = MigrationRequest {
        new_artifact: MigrationServiceV05.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 5, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

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

        builder.create()
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

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

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
    let request = MigrationRequest {
        new_artifact: MigrationServiceV05.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    // After the first migration step, version should be "0.2".
    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

    migration_service::v02::verify_schema(prefixed);

    // Request the same migration.
    let deadline_height = Height(DEADLINE_HEIGHT.0 * 2);
    let request = MigrationRequest {
        new_artifact: MigrationServiceV05.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    // Now we finally should have version "0.5".
    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 5, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

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

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    // Despite the fact that migration should fail, the transaction with request
    // should be executed successfully.
    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    let error = wait_for_migration_fail(&mut testkit, deadline_height, request);

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

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    // Obtain the expected migration hash and send confirmations from other nodes.
    let expected_hash = obtain_expected_hash(&mut testkit, &request);

    let migration_status = MigrationStatus(Ok(expected_hash));
    let migration_result = MigrationResult {
        request: request.clone(),
        result: migration_status,
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
    let migration_state = migration_state(&api, request.clone())
        .state
        .expect("State for requested migration is not stored");
    assert!(migration_state.state.is_pending());

    testkit.create_block_with_transactions(confirmations);

    // Now wait for migration success.
    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

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

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    // Obtain the expected migration hash and send confirmations from other nodes.
    let expected_hash = obtain_expected_hash(&mut testkit, &request);

    let migration_status = MigrationStatus(Ok(expected_hash));
    let migration_result = MigrationResult {
        request: request.clone(),
        result: migration_status,
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
    let migration_state = migration_state(&api, request.clone())
        .state
        .expect("State for requested migration is not stored");
    assert!(migration_state.state.is_pending());

    testkit.create_block_with_transactions(confirmations);

    // Now wait for migration timeout.
    wait_for_migration_timeout(&mut testkit, deadline_height, request);

    // After that check that schema did not change.
    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

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

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    // Obtain the expected migration hash and send confirmations from other nodes.
    let expected_hash = obtain_expected_hash(&mut testkit, &request);

    let migration_status = MigrationStatus(Ok(expected_hash));
    let migration_result = MigrationResult {
        request: request.clone(),
        result: migration_status,
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
        result: wrong_status,
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
    let migration_state = migration_state(&api, request.clone())
        .state
        .expect("State for requested migration is not stored");
    assert!(migration_state.state.is_pending());

    testkit.create_block_with_transactions(confirmations);

    // Now wait for migration timeout.
    let error = wait_for_migration_fail(&mut testkit, deadline_height, request);

    assert_eq!(
        error,
        ErrorMatch::from_fail(&MigrationError::StateHashDivergence).with_any_description()
    );

    // After that check that schema did not change.
    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

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

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

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
    let request = MigrationRequest {
        new_artifact: MigrationServiceV05_1.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    // After the first migration step, version should be "0.2".
    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 2, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

    migration_service::v02::verify_schema(prefixed);

    // Request the same migration.
    let deadline_height = Height(DEADLINE_HEIGHT.0 * 2);
    let request = MigrationRequest {
        new_artifact: MigrationServiceV05_1.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    // Now we should have version "0.5".
    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 5, 0),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

    migration_service::v05::verify_schema(prefixed);

    // Request the same migration for the third time.
    let deadline_height = Height(DEADLINE_HEIGHT.0 * 3);
    let request = MigrationRequest {
        new_artifact: MigrationServiceV05_1.artifact_id(),
        service: MigrationService::INSTANCE_NAME.into(),
        deadline_height,
    };

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    // Now we finally should have version "0.5.1".
    wait_for_migration_success(
        &mut testkit,
        deadline_height,
        request,
        Version::new(0, 5, 1),
    );

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigrationService::INSTANCE_NAME, &snapshot);

    // Data should not change.
    migration_service::v05::verify_schema(prefixed);
}
