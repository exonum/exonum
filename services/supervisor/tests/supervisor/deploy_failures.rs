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

//! Tests for the deployment process happening with errors, including
//! checks for correct deploy interruption upon a failure, re-deploying
//! artifact after the failure, and retrieving the status via API.
//!
//! Tests in this file are intended to be high-level and perform checks
//! in a way convenient for a situation, it is assumed that low-level
//! checks for supervisor mechanisms (e.g. sending request as a transaction
//! vs. sending request via API) are performed in other files.

use exonum::{
    crypto::Hash,
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Verified},
    runtime::{ExecutionError, SUPERVISOR_INSTANCE_ID},
};
use exonum_rust_runtime::ServiceFactory;
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};

use exonum_supervisor::{
    api::DeployInfoQuery, AsyncEventState, DeployRequest, DeployResult, Supervisor,
    SupervisorInterface,
};

use self::failing_runtime::{FailingRuntime, FailingRuntimeError};

mod failing_runtime {
    use std::str::FromStr;

    use exonum::merkledb::Snapshot;
    use exonum::runtime::{
        migrations::{InitMigrationError, MigrationScript},
        versioning::Version,
        ArtifactId, ExecutionContext, ExecutionError, InstanceSpec, InstanceStatus, Mailbox,
        MethodId, Runtime, WellKnownRuntime,
    };
    use exonum_derive::ExecutionFail;
    use futures::{Future, IntoFuture};

    /// Runtime which can fail within deployment.
    #[derive(Debug, Default)]
    pub(crate) struct FailingRuntime {
        // We have only one artifact that can be deployed,
        // and store its status as bool for simplicity.
        artifact_deployed: bool,
    }

    pub(crate) const FAILING_RUNTIME_ID: u32 = 3;

    impl FailingRuntime {
        pub const ARTIFACT_SHOULD_BE_DEPLOYED: &'static str = "success";
        pub const ARTIFACT_SHOULD_FAIL: &'static str = "fail";
        pub const ARTIFACT_VERSION: &'static str = "0.1.0";

        pub fn artifact_should_fail() -> ArtifactId {
            Self::artifact(Self::ARTIFACT_SHOULD_FAIL)
        }

        pub fn artifact_should_be_deployed() -> ArtifactId {
            Self::artifact(Self::ARTIFACT_SHOULD_BE_DEPLOYED)
        }

        fn artifact(artifact_name: &str) -> ArtifactId {
            // Parsing from string is just easier and requires less imports.
            let artifact_id_str =
                format!("{}:{}:{}", Self::ID, artifact_name, Self::ARTIFACT_VERSION);
            ArtifactId::from_str(&artifact_id_str).unwrap()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[derive(ExecutionFail)]
    #[execution_fail(kind = "runtime")]
    pub(crate) enum FailingRuntimeError {
        /// Generic deployment error.
        GenericError = 0,
        /// Deployment error upon a request.
        PlannedError = 1,
    }

    impl Runtime for FailingRuntime {
        fn deploy_artifact(
            &mut self,
            artifact: ArtifactId,
            _spec: Vec<u8>,
        ) -> Box<dyn Future<Item = (), Error = ExecutionError>> {
            let result = {
                if artifact.runtime_id != FAILING_RUNTIME_ID {
                    Err(FailingRuntimeError::GenericError.into())
                } else if artifact.name.as_str() == Self::ARTIFACT_SHOULD_FAIL {
                    Err(FailingRuntimeError::PlannedError.into())
                } else if artifact.name.as_str() == Self::ARTIFACT_SHOULD_BE_DEPLOYED {
                    Ok(())
                } else {
                    panic!("Attempt to deploy an artifact not expected by failing runtime")
                }
            };

            Box::new(result.into_future())
        }

        fn is_artifact_deployed(&self, id: &ArtifactId) -> bool {
            if id.runtime_id == FAILING_RUNTIME_ID
                && id.name.as_str() == Self::ARTIFACT_SHOULD_BE_DEPLOYED
            {
                return self.artifact_deployed;
            }
            // All the other artifacts are not deployed in any sense.
            false
        }

        /// Initiates adding a new service and sets the counter value for this.
        fn initiate_adding_service(
            &self,
            _context: ExecutionContext<'_>,
            _artifact: &ArtifactId,
            _params: Vec<u8>,
        ) -> Result<(), ExecutionError> {
            unimplemented!("This runtime does not support service instantiation");
        }

        fn initiate_resuming_service(
            &self,
            _context: ExecutionContext<'_>,
            _artifact: &ArtifactId,
            _params: Vec<u8>,
        ) -> Result<(), ExecutionError> {
            unimplemented!("This runtime does not support service resuming");
        }

        /// Commits status for the `SampleService` instance with the specified ID.
        fn update_service_status(
            &mut self,
            _snapshot: &dyn Snapshot,
            _spec: &InstanceSpec,
            _status: &InstanceStatus,
        ) {
            unimplemented!("This runtime does not support service instantiation");
        }

        fn migrate(
            &self,
            _new_artifact: &ArtifactId,
            _data_version: &Version,
        ) -> Result<Option<MigrationScript>, InitMigrationError> {
            unimplemented!("This runtime does not support data migration");
        }

        fn execute(
            &self,
            _context: ExecutionContext<'_>,
            _method_id: MethodId,
            _payload: &[u8],
        ) -> Result<(), ExecutionError> {
            unimplemented!("This runtime does not support service instantiation");
        }

        fn before_transactions(
            &self,
            _context: ExecutionContext<'_>,
        ) -> Result<(), ExecutionError> {
            Ok(())
        }

        fn after_transactions(&self, _context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
            Ok(())
        }

        fn after_commit(&mut self, _snapshot: &dyn Snapshot, _mailbox: &mut Mailbox) {}
    }

    impl WellKnownRuntime for FailingRuntime {
        const ID: u32 = FAILING_RUNTIME_ID;
    }
}

// For most of the tests 2 validators is enough: one is us, and one represents the rest of network.
const VALIDATORS_AMOUNT: u16 = 2;
const VALIDATOR_OTHER: ValidatorId = ValidatorId(1);
const DEPLOY_HEIGHT: Height = Height(3);
// ^-- 1 for sending request, 1 for sending confirmation, 1 for approval

/// Creates a new testkit with simple supervisor and failing runtime.
fn testkit_with_failing_runtime(validator_count: u16) -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_validators(validator_count)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .with_additional_runtime(FailingRuntime::default())
        .build()
}

/// Creates a `DeployResult` transaction for `ValidatorId(1)`.
fn build_result_transaction(
    testkit: &TestKit,
    request: &DeployRequest,
    result: Result<(), ExecutionError>,
) -> Verified<AnyTx> {
    testkit
        .network()
        .validators()
        .iter()
        .find(|validator| validator.validator_id() == Some(VALIDATOR_OTHER))
        .map(|validator| {
            validator.service_keypair().report_deploy_result(
                SUPERVISOR_INSTANCE_ID,
                DeployResult {
                    request: request.clone(),
                    result: result.into(),
                },
            )
        })
        .unwrap()
}

/// Creates `AsyncEventState::Failed` for planned error of `FailingRuntime`.
fn fail_state(height: Height) -> AsyncEventState {
    AsyncEventState::Failed {
        error: FailingRuntimeError::PlannedError.into(),
        height,
    }
}

/// Sends a deploy request through API.
fn send_deploy_request(api: &TestKitApi, request: &DeployRequest) -> Hash {
    api.private(ApiKind::Service("supervisor"))
        .query(request)
        .post("deploy-artifact")
        .expect("Call for `deploy-artifact` API endpoint failed")
}

/// Gets a deploy status for a certain request.
fn get_deploy_status(api: &TestKitApi, request: &DeployRequest) -> AsyncEventState {
    let query = DeployInfoQuery::from(request.clone());
    api.private(ApiKind::Service("supervisor"))
        .query(&query)
        .get("deploy-status")
        .expect("Call for `deploy-status` API endpoint failed")
}

// Verifies that two `AsyncEventState` objects are equal, behaving similar
// to `assert_eq`.
// This function is required, since `AsyncEventState` doesn't implement `PartialEq`.
fn assert_deploy_state(actual: AsyncEventState, expected: AsyncEventState) {
    use AsyncEventState::*;
    match (&actual, &expected) {
        // Same variants, no actions needed.
        (Pending, Pending) | (Succeed, Succeed) | (Timeout, Timeout) => {}
        // Failures caused by error, check that inner content equals.
        (left @ Failed { .. }, right @ Failed { .. }) => {
            let assertion_failure_msg = format!(
                "Different deploy states, got {:?}, expected {:?}",
                actual, expected
            );

            // Compare height.
            assert_eq!(
                left.height().unwrap(),
                right.height().unwrap(),
                "{}",
                &assertion_failure_msg,
            );

            // Compare errors, casting the expected one to match.
            assert_eq!(
                left.execution_error().unwrap(),
                right.execution_error().unwrap().to_match(),
                "{}",
                &assertion_failure_msg,
            )
        }
        // Non-symmetric variants.
        _ => {
            panic!(
                "Deploy states are not equal, got {:?}, expected {:?}",
                actual, expected
            );
        }
    }
}

/// Test for self-checking the `FailingRuntime` concept and `deploy-status` endpoint.
/// This test checks the normal conditions: deploy for our node succeed, was confirmed
/// by the other node and should be deployed within network.
#[test]
fn deploy_success() {
    let mut testkit = testkit_with_failing_runtime(VALIDATORS_AMOUNT);
    let api = testkit.api();

    let deploy_request = DeployRequest {
        artifact: FailingRuntime::artifact_should_be_deployed(),
        spec: Vec::new(),
        deadline_height: DEPLOY_HEIGHT,
    };

    let tx_hash = send_deploy_request(&api, &deploy_request);
    let block = testkit.create_block();
    block[tx_hash].status().unwrap();

    // Check that request is now pending.
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Pending);

    // Confirm deploy.
    let result = Ok(());
    let deploy_confirmation = build_result_transaction(&testkit, &deploy_request, result);
    testkit.create_block_with_transaction(deploy_confirmation);

    testkit.create_blocks_until(DEPLOY_HEIGHT.next());

    // Check that status is `Succeed`.
    let api = testkit.api();
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Succeed);
}

/// Checks that deployment fails if there was no enough confirmations
/// when the deadline height was achieved.
#[test]
fn deploy_failure_because_not_confirmed() {
    let mut testkit = testkit_with_failing_runtime(VALIDATORS_AMOUNT);
    let api = testkit.api();

    let deploy_request = DeployRequest {
        artifact: FailingRuntime::artifact_should_be_deployed(),
        spec: Vec::new(),
        deadline_height: DEPLOY_HEIGHT,
    };

    let tx_hash = send_deploy_request(&api, &deploy_request);
    let block = testkit.create_block();
    block[tx_hash].status().unwrap();

    // Check that request is now pending.
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Pending);

    // Do NOT confirm a deploy.
    testkit.create_blocks_until(DEPLOY_HEIGHT.next());

    // Check that status is `Failed` at the deadline height.
    let api = testkit.api();
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Timeout);
}

/// Checks that if deployment attempt fails for our node, the deploy
/// is failed despite the confirmation from other node.
#[test]
fn deploy_failure_because_cannot_deploy() {
    let mut testkit = testkit_with_failing_runtime(VALIDATORS_AMOUNT);
    let api = testkit.api();

    let deploy_request = DeployRequest {
        artifact: FailingRuntime::artifact_should_fail(),
        spec: Vec::new(),
        deadline_height: DEPLOY_HEIGHT,
    };

    let tx_hash = send_deploy_request(&api, &deploy_request);
    let block = testkit.create_block();
    block[tx_hash].status().unwrap();

    // Check that request is now pending.
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Pending);

    // Confirm deploy (it should not affect the overall failure).
    let result = Ok(());
    let deploy_confirmation = build_result_transaction(&testkit, &deploy_request, result);
    testkit.create_block_with_transaction(deploy_confirmation);

    testkit.create_blocks_until(DEPLOY_HEIGHT.next());

    // Check that status is `Failed` right after node will try to perform deploy:
    // the deployment is scheduled itself for the next block, but is performed in
    // `after_commit`, thus height is 2.
    let api = testkit.api();
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, fail_state(Height(2)));
}

/// This test has the same idea as `deploy_failure_because_not_confirmed`,
/// but is more low-level: we ensure that deploy not only ends in a failure
/// if node does not perform deployment attempts at every block.
///
/// Motivation: there was a bug which caused `Supervisor` to attempt deployments
/// every block until the deadline height.
#[test]
fn deploy_failure_check_no_extra_actions() {
    // Choose some bigger height to verify that no extra actions are performed
    // after deployment activities and before deadline height.
    const BIGGER_DEPLOY_HEIGHT: Height = Height(10);

    let mut testkit = testkit_with_failing_runtime(VALIDATORS_AMOUNT);
    let api = testkit.api();

    let deploy_request = DeployRequest {
        artifact: FailingRuntime::artifact_should_fail(),
        spec: Vec::new(),
        deadline_height: BIGGER_DEPLOY_HEIGHT,
    };

    let tx_hash = send_deploy_request(&api, &deploy_request);
    let block = testkit.create_block();
    block[tx_hash].status().unwrap();

    // Check that request is now pending.
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Pending);

    // Confirm deploy (it should not affect the overall failure).
    let result = Ok(());
    let deploy_confirmation = build_result_transaction(&testkit, &deploy_request, result);
    testkit.create_block_with_transaction(deploy_confirmation);

    // Now, in `after_commit` we should attempt to deploy an artifact, and send result tx.
    // It is expected to appear in the next block.
    let block = testkit.create_block();
    assert_eq!(block.transactions.len(), 1);

    // Check that deployment is already marked as failed.
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, fail_state(Height(2)));

    // Ensure that there are no more transactions until the deadline height.
    // This is sufficient, since after any deploy attempt we are sending a transaction
    // despite of result.
    // Thus, no transactions => no deploy attempts.
    while testkit.height() < BIGGER_DEPLOY_HEIGHT.next() {
        let block = testkit.create_block();
        assert_eq!(block.transactions.len(), 0);
    }

    // Check the deployment status again (after the deadline),
    // it should not change.
    let api = testkit.api();
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, fail_state(Height(2)));
}

/// Checks that if other node sends a failure report, deployment fails as well.
#[test]
fn deploy_failure_because_other_node_cannot_deploy() {
    let mut testkit = testkit_with_failing_runtime(VALIDATORS_AMOUNT);
    let api = testkit.api();

    let deploy_request = DeployRequest {
        artifact: FailingRuntime::artifact_should_be_deployed(),
        spec: Vec::new(),
        deadline_height: DEPLOY_HEIGHT,
    };

    let tx_hash = send_deploy_request(&api, &deploy_request);
    let block = testkit.create_block();
    block[tx_hash].status().unwrap();

    // Check that request is now pending.
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Pending);

    // Send a notification that a node can not deploy the artifact.
    let error: ExecutionError = FailingRuntimeError::GenericError.into();
    let deploy_confirmation =
        build_result_transaction(&testkit, &deploy_request, Err(error.clone()));
    testkit.create_block_with_transaction(deploy_confirmation);

    testkit.create_blocks_until(DEPLOY_HEIGHT.next());

    // Check that status is `Failed` on the same height when failure report
    // was received from other node (in the second block, which corresponds to height 1).
    let api = testkit.api();
    let state = get_deploy_status(&api, &deploy_request);
    let fail_state = AsyncEventState::Failed {
        height: Height(1),
        error,
    };
    assert_deploy_state(state, fail_state);
}

/// Checks that after unsuccessful deploy attempt we can perform another try and it can
/// result in a success.
#[test]
fn deploy_successfully_after_failure() {
    // 1. Perform the same routine as in `deploy_failure_because_other_node_cannot_deploy`:
    // - attempt to deploy an artifact that can be deployed;
    // - receive failure report from the other node;
    // - ensure that deployment is failed.

    let mut testkit = testkit_with_failing_runtime(VALIDATORS_AMOUNT);
    let api = testkit.api();

    let deploy_request = DeployRequest {
        artifact: FailingRuntime::artifact_should_be_deployed(),
        spec: Vec::new(),
        deadline_height: DEPLOY_HEIGHT,
    };

    let tx_hash = send_deploy_request(&api, &deploy_request);
    let block = testkit.create_block();
    block[tx_hash].status().unwrap();

    // Check that request is now pending.
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Pending);

    // Send a notification that a node can not deploy the artifact.
    let error: ExecutionError = FailingRuntimeError::GenericError.into();
    let deploy_confirmation =
        build_result_transaction(&testkit, &deploy_request, Err(error.clone()));
    testkit.create_block_with_transaction(deploy_confirmation);

    testkit.create_blocks_until(DEPLOY_HEIGHT.next());

    // Check that status is `Failed` on the same height when failure report
    // was received from other node (in the second block, which corresponds to height 1).
    let api = testkit.api();
    let state = get_deploy_status(&api, &deploy_request);
    let fail_state = AsyncEventState::Failed {
        height: Height(1),
        error,
    };
    assert_deploy_state(state, fail_state);

    // 2. Update the deadline height and perform the same routine as in `deploy_success`:
    // - attempt to deploy the same artifact;
    // - receive the confirmation;
    // - ensure that artifact is now deployed.

    // +1 is required since we've been creating blocks until `DEPLOY_HEIGHT.next()`.
    const NEW_DEPLOY_HEIGHT: Height = Height(DEPLOY_HEIGHT.0 * 2 + 1);

    let api = testkit.api();

    let deploy_request = DeployRequest {
        artifact: FailingRuntime::artifact_should_be_deployed(),
        spec: Vec::new(),
        deadline_height: NEW_DEPLOY_HEIGHT,
    };

    let tx_hash = send_deploy_request(&api, &deploy_request);
    let block = testkit.create_block();
    block[tx_hash].status().unwrap();

    // Check that request is now pending.
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Pending);

    // Confirm deploy.
    let result = Ok(());
    let deploy_confirmation = build_result_transaction(&testkit, &deploy_request, result);
    testkit.create_block_with_transaction(deploy_confirmation);

    testkit.create_blocks_until(NEW_DEPLOY_HEIGHT.next());

    // Check that status is `Succeed`.
    let api = testkit.api();
    let state = get_deploy_status(&api, &deploy_request);
    assert_deploy_state(state, AsyncEventState::Succeed);
}

/// Checks that `deploy-status` returns `NotFound` for unknown requests.
#[test]
fn not_requested_deploy_status() {
    let mut testkit = testkit_with_failing_runtime(VALIDATORS_AMOUNT);
    let api = testkit.api();

    let deploy_request = DeployRequest {
        artifact: FailingRuntime::artifact_should_be_deployed(),
        spec: Vec::new(),
        deadline_height: DEPLOY_HEIGHT,
    };

    let query = DeployInfoQuery::from(deploy_request);
    let error = api
        .private(ApiKind::Service("supervisor"))
        .query(&query)
        .get::<AsyncEventState>("deploy-status")
        .expect_err("Call for `deploy-status` API endpoint succeed, but was expected to fail");

    assert_eq!(u16::from(error.http_code), 404);
}
