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
    helpers::Height,
    merkledb::access::Prefixed,
    runtime::{ErrorMatch, ExecutionError, InstanceId},
};
use exonum_rust_runtime::{DefaultInstance, ServiceFactory};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};

use exonum_supervisor::{
    AsyncEventState, ConfigPropose, MigrationError, MigrationInfoQuery, MigrationRequest,
    ProcessStateResponse, Supervisor,
};

use crate::service_lifecycle::execute_transaction;

use migration_service::{MigratedService, MigratedServiceV02, MigratedServiceV05};

mod migration_service;

fn testkit_with_supervisor_and_service(validator_count: u16) -> TestKit {
    // Initialize builder;
    let builder = TestKitBuilder::validator().with_validators(validator_count);

    // Add supervisor.
    let builder = builder
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple());

    // Add MigratedService with running instance.
    let builder = builder.with_default_rust_service(MigratedService);

    // Add migrating artifact for version 0.2.
    let builder = builder
        .with_migrating_rust_service(MigratedServiceV02)
        .with_artifact(MigratedServiceV02.artifact_id());

    // Add artifact for version 0.5.
    let builder = builder
        .with_migrating_rust_service(MigratedServiceV05)
        .with_artifact(MigratedServiceV05.artifact_id());

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
fn migration_state(api: &TestKitApi, request: MigrationRequest) -> ProcessStateResponse {
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

fn wait_for_migration_success(
    testkit: &mut TestKit,
    deadline_height: Height,
    request: MigrationRequest,
) {
    let mut success = false;
    while testkit.height() <= deadline_height.next() {
        testkit.create_block();
        let api = testkit.api();
        let migration_state = migration_state(&api, request.clone())
            .state
            .expect("State for requested migration is not stored");

        match migration_state {
            AsyncEventState::Pending => {
                // Not ready yet.
            }
            AsyncEventState::Succeed => {
                // Migration completed.
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

        match migration_state {
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

const DEADLINE_HEIGHT: Height = Height(5);

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
    stop_service(&mut testkit, MigratedService::INSTANCE_ID);

    // Request migration.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigratedServiceV02.artifact_id(),
        service: MigratedService::INSTANCE_NAME.into(),
        deadline_height,
    };

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    wait_for_migration_success(&mut testkit, deadline_height, request);

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigratedService::INSTANCE_NAME, &snapshot);

    migration_service::v02::verify_schema(prefixed);
}

/// This test applies two migrations to one service, one after another.
#[test]
fn migration_two_scripts_sequential() {
    let mut testkit = testkit_with_supervisor_and_service(1);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigratedService::INSTANCE_ID);

    // Request migration to 0.2.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigratedServiceV02.artifact_id(),
        service: MigratedService::INSTANCE_NAME.into(),
        deadline_height,
    };

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    wait_for_migration_success(&mut testkit, deadline_height, request);

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigratedService::INSTANCE_NAME, &snapshot);

    migration_service::v02::verify_schema(prefixed);

    // Request migration to 0.5.
    let deadline_height = Height(DEADLINE_HEIGHT.0 * 2);
    let request = MigrationRequest {
        new_artifact: MigratedServiceV05.artifact_id(),
        service: MigratedService::INSTANCE_NAME.into(),
        deadline_height,
    };

    let api = testkit.api();
    let tx_hash = request_migration(&api, request.clone());
    let block = testkit.create_block();

    block[tx_hash]
        .status()
        .expect("Transaction should be executed successfully");

    wait_for_migration_success(&mut testkit, deadline_height, request);

    let snapshot = testkit.snapshot();
    let prefixed = Prefixed::new(MigratedService::INSTANCE_NAME, &snapshot);

    migration_service::v05::verify_schema(prefixed);
}

/// This test checks that attempt to request a complex migration (which will require
/// multiple migration scripts to be executed) results in a migration failure.
#[test]
fn comlplex_migration_fails() {
    let mut testkit = testkit_with_supervisor_and_service(1);

    // Stop service instance before running the migration.
    stop_service(&mut testkit, MigratedService::INSTANCE_ID);

    // Request migration.
    let deadline_height = DEADLINE_HEIGHT;
    let request = MigrationRequest {
        new_artifact: MigratedServiceV05.artifact_id(),
        service: MigratedService::INSTANCE_NAME.into(),
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
        ErrorMatch::from_fail(&MigrationError::ComplexMigration).with_any_description()
    )
}
