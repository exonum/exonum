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

use exonum::messages::{AnyTx, Verified};
use exonum_rust_runtime::{
    DefaultInstance, ErrorMatch, ExecutionError, InstanceId, ServiceFactory, SnapshotExt,
    SUPERVISOR_INSTANCE_ID,
};
use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};

use exonum_supervisor::{ConfigPropose, Error, Supervisor};

use crate::inc::IncService;

/// Creates block with the specified transaction and returns its execution result.
fn execute_transaction(testkit: &mut TestKit, tx: Verified<AnyTx>) -> Result<(), ExecutionError> {
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
fn start_inc_service(testkit: &mut TestKit) -> InstanceId {
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
    let service_info = testkit
        .snapshot()
        .for_dispatcher()
        .get_instance(IncService::INSTANCE_NAME)
        .unwrap();
    service_info.spec.id
}

#[test]
fn start_stop_inc_service() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance_id = start_inc_service(&mut testkit);
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
        ErrorMatch::from_fail(&Error::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing("Instance with the specified ID is absent.")
    )
}

#[test]
fn duplicate_stop_service_request() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    let instance_id = start_inc_service(&mut testkit);
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
        ErrorMatch::from_fail(&Error::MalformedConfigPropose)
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

    let instance_id = start_inc_service(&mut testkit);
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
        ErrorMatch::from_fail(&Error::MalformedConfigPropose)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_description_containing(
                "Discarded an attempt to stop the already stopped service instance"
            )
    )
}
