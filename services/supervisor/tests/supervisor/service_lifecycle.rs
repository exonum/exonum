// Copyright 2019 The Exonum Team
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
    runtime::{rust::ServiceFactory, ExecutionError},
};
use exonum_testkit::{TestKit, TestKitBuilder};

use exonum_supervisor::{ConfigPropose, Supervisor};

use crate::{inc::IncService, utils::latest_assigned_instance_id};

/// Creates block with the specified transaction and returns its execution result.
fn execute_transaction(testkit: &mut TestKit, tx: Verified<AnyTx>) -> Result<(), ExecutionError> {
    testkit.create_block_with_transaction(tx).transactions[0]
        .status()
        .map_err(Clone::clone)
}

fn create_testkit() -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_rust_service(Supervisor)
        .with_rust_service(IncService)
        .with_artifact(Supervisor.artifact_id())
        .with_artifact(IncService.artifact_id())
        .with_instance(Supervisor::simple())
        .create()
}

#[test]
fn start_stop_inc_service() {
    let mut testkit = create_testkit();
    let keypair = testkit.us().service_keypair();

    // Start service instance and get its ID.
    execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(0)
            .start_service(IncService.artifact_id().into(), "inc", Vec::default())
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .unwrap();
    let instance_id = latest_assigned_instance_id(&testkit).unwrap();
    // Stop service instance.
    execute_transaction(
        &mut testkit,
        ConfigPropose::immediate(1)
            .stop_service(instance_id)
            .sign_for_supervisor(keypair.0, &keypair.1),
    )
    .unwrap()
}
