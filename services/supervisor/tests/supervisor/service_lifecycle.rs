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

//! Tests for the phases of the service life cycle, including starting and stopping service instances.

use exonum::{
    messages::{AnyTx, Verified},
    runtime::{ErrorMatch, ExecutionError, InstanceState, SnapshotExt, SUPERVISOR_INSTANCE_ID},
};
use exonum_rust_runtime::{DefaultInstance, ServiceFactory};
use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};

use crate::inc::IncService;
use exonum_supervisor::{ConfigPropose, ConfigurationError, Supervisor};

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
fn is_inc_service_api_available(testkit: &mut TestKit) -> bool {
    testkit
        .api()
        .public(ApiKind::Service(IncService::INSTANCE_NAME))
        .get::<()>("v1/ping")
        .is_ok()
}

fn create_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with_rust_service(Supervisor)
        .with_rust_service(IncService)
        .with_artifact(Supervisor.artifact_id())
        .with_artifact(IncService.artifact_id())
        .with_instance(Supervisor::simple())
        .create()
}

/// Starts service instance and gets its ID
fn start_inc_service(testkit: &mut TestKit) -> InstanceState {
    let keypair = testkit.us().service_keypair();
    // Start `inc` service instance
    execute_transaction(
        testkit,
        ConfigPropose::immediate(0)
            .start_service(
                IncService.artifact_id(),
                IncService::INSTANCE_NAME,
                Vec::default(),
            )
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect("Start service transaction should be processed");
    // Get started service instance ID.
    testkit
        .snapshot()
        .for_dispatcher()
        .get_instance(IncService::INSTANCE_NAME)
        .unwrap()
}

#[test]
fn start_stop_inc_service() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance_id = start_inc_service(&mut testkit).spec.id;
    assert!(
        is_inc_service_api_available(&mut testkit),
        "Inc service API should be available after starting."
    );
    // Stop service instance.
    execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(1)
            .stop_service(instance_id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect("Stop service transaction should be processed");
    assert!(
        !is_inc_service_api_available(&mut testkit),
        "Inc service API should not be available after stopping."
    );
}

#[test]
fn stop_non_existent_service() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance_id = 2;
    let actual_err = execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(1)
            .stop_service(instance_id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing("Instance with the specified ID is absent.")
    )
}

#[test]
fn duplicate_stop_service_request() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance_id = start_inc_service(&mut testkit).spec.id;
    // An attempt to stop service twice.
    let actual_err = execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(1)
            .stop_service(instance_id)
            .stop_service(instance_id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing(
                "Discarded multiple instances with the same name in one request."
            )
    )
}

#[test]
fn stop_already_stopped_service() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance_id = start_inc_service(&mut testkit).spec.id;
    // Stop service instance.
    execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(1)
            .stop_service(instance_id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect("Transaction should be processed");
    // Second attempt to stop service instance.
    let actual_err = execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(2)
            .stop_service(instance_id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing(
                "Discarded an attempt to stop the already stopped service instance"
            )
    )
}

#[test]
fn resume_stopped_service() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance = start_inc_service(&mut testkit);
    // Stop service instance.
    execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(1)
            .stop_service(instance.spec.id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect("Transaction should be processed");

    // Resume service instance.
    execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(2)
            .resume_service(instance.spec.id, instance.spec.artifact, Vec::default())
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect("Transaction should be processed");

    // Check resumed service API.
    assert!(is_inc_service_api_available(&mut testkit));
}

#[test]
fn resume_active_service() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance = start_inc_service(&mut testkit);
    // Resume service instance.
    let actual_err = execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(1)
            .resume_service(instance.spec.id, instance.spec.artifact, Vec::default())
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing(
                "Discarded an attempt to resume not stopped service instance"
            )
    )
}

#[test]
fn resume_service_with_different_artifact_name() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance = start_inc_service(&mut testkit);
    // Stop service instance.
    execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(1)
            .stop_service(instance.spec.id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect("Transaction should be processed");

    // Resume service instance.
    let mut artifact = instance.spec.artifact;
    artifact.name = "inc2".to_owned();
    let actual_err = execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(2)
            .resume_service(instance.spec.id, artifact, Vec::default())
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing(
                "Discarded an attempt to resume service with different artifact name"
            )
    )
}

#[test]
fn resume_service_with_different_artifact_version() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance = start_inc_service(&mut testkit);
    // Stop service instance.
    execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(1)
            .stop_service(instance.spec.id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect("Transaction should be processed");

    // Resume service instance.
    let mut artifact = instance.spec.artifact;
    artifact.version.patch += 1;
    let actual_err = execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(2)
            .resume_service(instance.spec.id, artifact, Vec::default())
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing(
                "Discarded an attempt to resume service with incorrect artifact version"
            )
    )
}

#[test]
fn multiple_stop_resume_requests() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let spec = start_inc_service(&mut testkit).spec;

    // Config proposal with two requests for single service instance.
    let actual_err = execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(2)
            .stop_service(spec.id)
            .resume_service(spec.id, spec.artifact, Vec::default())
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .expect_err("Transaction shouldn't be processed");

    assert_eq!(
        actual_err,
        ErrorMatch::from_fail(&ConfigurationError::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing(
                "Discarded multiple instances with the same name in one request"
            )
    )
}
