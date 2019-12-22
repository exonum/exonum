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

use exonum::{
    api, crypto,
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Verified},
    runtime::{
        rust::{RustRuntime, ServiceFactory},
        ArtifactId, ErrorMatch, InstanceId, RuntimeIdentifier, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_merkledb::ObjectHash;
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};

use exonum_supervisor::{
    ConfigPropose, DeployConfirmation, DeployRequest, Error as TxError, Supervisor,
    SupervisorInterface,
};

use crate::{
    inc::{IncInterface, IncService, SERVICE_ID, SERVICE_NAME},
    utils::build_confirmation_transactions,
};

mod config;
mod config_api;
mod consensus_config;
mod inc;
mod proto;
mod service_lifecycle;
mod supervisor_config;
mod utils;

fn default_artifact() -> ArtifactId {
    IncService.artifact_id()
}

fn assert_count(api: &TestKitApi, service_name: &'static str, expected_count: u64) {
    let real_count: u64 = api
        .public(ApiKind::Service(service_name))
        .get("v1/counter")
        .unwrap();
    assert_eq!(real_count, expected_count);
}

/// Check that the service's counter isn't started yet (no Inc txs were received).
fn assert_count_is_not_set(api: &TestKitApi, service_name: &'static str) {
    let response: api::Result<u64> = api.public(ApiKind::Service(service_name)).get("v1/counter");
    assert!(response.is_err());
}

fn artifact_exists(api: &TestKitApi, name: &str) -> bool {
    let artifacts = &api.exonum_api().services().artifacts;
    artifacts.iter().any(|a| a.name == name)
}

fn service_instance_exists(api: &TestKitApi, name: &str) -> bool {
    let services = &api.exonum_api().services().services;
    services.iter().any(|s| s.spec.name == name)
}

fn find_instance_id(api: &TestKitApi, instance_name: &str) -> InstanceId {
    let services = &api.exonum_api().services().services;
    services
        .iter()
        .find(|service| service.spec.name == instance_name)
        .expect("Can't find the instance")
        .spec
        .id
}

fn deploy_artifact(api: &TestKitApi, request: DeployRequest) -> crypto::Hash {
    let hash: crypto::Hash = api
        .private(ApiKind::Service("supervisor"))
        .query(&request)
        .post("deploy-artifact")
        .unwrap();
    hash
}

fn deploy_artifact_manually(
    testkit: &mut TestKit,
    request: &DeployRequest,
    validator_id: ValidatorId,
) -> crypto::Hash {
    let keypair = testkit.validator(validator_id).service_keypair();
    let signed_request =
        keypair.request_artifact_deploy(SUPERVISOR_INSTANCE_ID, request.to_owned());
    let request_hash = signed_request.object_hash();
    testkit.add_tx(signed_request);
    request_hash
}

fn start_service(api: &TestKitApi, request: ConfigPropose) -> crypto::Hash {
    // Even though this method sends a config proposal, it's *intended* to start
    // services (so the callee-side code will be more readable).
    // However, this convention is up to test writers.

    let hash: crypto::Hash = api
        .private(ApiKind::Service("supervisor"))
        .query(&request)
        .post("propose-config")
        .unwrap();
    hash
}

fn start_service_manually(
    testkit: &mut TestKit,
    request: &ConfigPropose,
    validator_id: ValidatorId,
) -> crypto::Hash {
    let keypair = testkit.validator(validator_id).service_keypair();
    let signed_request = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, request.to_owned());
    let request_hash = signed_request.object_hash();
    testkit.add_tx(signed_request);
    request_hash
}

fn deploy_confirmation(
    testkit: &TestKit,
    request: &DeployRequest,
    validator_id: ValidatorId,
) -> Verified<AnyTx> {
    let confirmation = request.to_owned().into();
    testkit
        .validator(validator_id)
        .service_keypair()
        .confirm_artifact_deploy(SUPERVISOR_INSTANCE_ID, confirmation)
}

fn deploy_confirmation_hash(
    testkit: &TestKit,
    request: &DeployRequest,
    validator_id: ValidatorId,
) -> crypto::Hash {
    let confirmation_signed = deploy_confirmation(testkit, request, validator_id);
    confirmation_signed.object_hash()
}

fn deploy_confirmation_hash_default(testkit: &TestKit, request: &DeployRequest) -> crypto::Hash {
    deploy_confirmation_hash(testkit, request, ValidatorId(0))
}

fn deploy_request(artifact: ArtifactId, deadline_height: Height) -> DeployRequest {
    DeployRequest {
        artifact,
        spec: Vec::default(),
        deadline_height,
    }
}

fn start_service_request(
    artifact: ArtifactId,
    name: impl Into<String>,
    deadline_height: Height,
) -> ConfigPropose {
    ConfigPropose::new(0, deadline_height).start_service(artifact, name, Vec::default())
}

fn deploy_default(testkit: &mut TestKit) {
    let artifact = default_artifact();
    let api = testkit.api();

    assert!(!artifact_exists(&api, &artifact.name));

    let request = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation_hash = deploy_confirmation_hash_default(testkit, &request);
    let hash = deploy_artifact(&api, request);
    testkit.create_block();

    api.exonum_api().assert_tx_success(hash);

    // Confirmation is ready.
    assert!(testkit.is_tx_in_pool(&deploy_confirmation_hash));

    testkit.create_block();

    // Confirmation is gone now.
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_hash));

    let api = testkit.api(); // update the API
    assert!(artifact_exists(&api, &artifact.name));
}

fn start_service_instance(testkit: &mut TestKit, instance_name: &str) -> InstanceId {
    let api = testkit.api();
    assert!(!service_instance_exists(&api, instance_name));
    let request = start_service_request(default_artifact(), instance_name, testkit.height().next());
    let hash = start_service(&api, request);
    testkit.create_block();
    api.exonum_api().assert_tx_success(hash);

    let api = testkit.api(); // Update the API
    assert!(service_instance_exists(&api, instance_name));
    find_instance_id(&api, instance_name)
}

fn testkit_with_inc_service() -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .with_rust_service(IncService)
        .create()
}

fn testkit_with_inc_service_and_n_validators(n: u16) -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .with_rust_service(IncService)
        .with_validators(n)
        .create()
}

fn testkit_with_inc_service_and_two_validators() -> TestKit {
    testkit_with_inc_service_and_n_validators(2)
}

fn testkit_with_inc_service_auditor_validator() -> TestKit {
    TestKitBuilder::auditor()
        .with_logger()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .with_rust_service(IncService)
        .with_validators(1)
        .create()
}

fn testkit_with_inc_service_and_static_instance() -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .with_default_rust_service(IncService)
        .create()
}

fn add_available_services(runtime: RustRuntime) -> RustRuntime {
    runtime.with_factory(IncService).with_factory(Supervisor)
}

/// Just test that the Inc service works as intended.
#[test]
fn test_static_service() {
    let mut testkit = testkit_with_inc_service_and_static_instance();
    let api = testkit.api();

    assert_count_is_not_set(&api, SERVICE_NAME);

    let keypair = crypto::gen_keypair();
    api.send(keypair.inc(SERVICE_ID, 0));
    testkit.create_block();
    assert_count(&api, SERVICE_NAME, 1);
    api.send(keypair.inc(SERVICE_ID, 1));
    testkit.create_block();
    assert_count(&api, SERVICE_NAME, 2);
}

/// Test a normal dynamic service workflow with one validator.
#[test]
fn test_dynamic_service_normal_workflow() {
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);
    let instance_name = "test_basics";
    let instance_id = start_service_instance(&mut testkit, instance_name);
    let api = testkit.api();

    assert_count_is_not_set(&api, instance_name);

    let keypair = crypto::gen_keypair();
    api.send(keypair.inc(instance_id, 0));
    testkit.create_block();
    assert_count(&api, instance_name, 1);

    api.send(keypair.inc(instance_id, 1));
    testkit.create_block();
    assert_count(&api, instance_name, 2);
}

#[test]
fn test_artifact_deploy_with_already_passed_deadline_height() {
    let mut testkit = testkit_with_inc_service();

    // We skip to Height(1) ...
    testkit.create_block();

    // ... but set Height(0) as a deadline.
    let bad_deadline_height = testkit.height().previous();

    let artifact = default_artifact();
    let api = testkit.api();

    let request = deploy_request(artifact.clone(), bad_deadline_height);
    let deploy_confirmation_hash = deploy_confirmation_hash_default(&testkit, &request);
    let hash = deploy_artifact(&api, request);
    testkit.create_block();

    assert!(!artifact_exists(&api, &artifact.name));

    // No confirmation was generated
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_hash));
    let system_api = api.exonum_api();
    let expected_err =
        ErrorMatch::from_fail(&TxError::ActualFromIsPast).for_service(SUPERVISOR_INSTANCE_ID);
    system_api.assert_tx_status(hash, Err(&expected_err));
}

#[test]
fn test_start_service_instance_with_already_passed_deadline_height() {
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    let api = testkit.api();
    let artifact = default_artifact();
    let instance_name = "inc_test";
    let bad_deadline_height = testkit.height().previous();
    let request = start_service_request(artifact, instance_name, bad_deadline_height);
    let hash = start_service(&api, request);
    testkit.create_block();

    let system_api = api.exonum_api();
    let expected_err =
        ErrorMatch::from_fail(&TxError::ActualFromIsPast).for_service(SUPERVISOR_INSTANCE_ID);
    system_api.assert_tx_status(hash, Err(&expected_err));
}

#[test]
fn test_try_run_unregistered_service_instance() {
    let mut testkit = testkit_with_inc_service();
    let api = testkit.api();

    // Deliberately missing the DeployRequest step.

    let instance_name = "wont_run";
    let request = start_service_request(default_artifact(), instance_name.to_owned(), Height(1000));
    let hash = start_service(&api, request);
    testkit.create_block();

    let system_api = api.exonum_api();
    let expected_err = ErrorMatch::from_fail(&TxError::UnknownArtifact)
        .for_service(SUPERVISOR_INSTANCE_ID)
        .with_any_description();
    system_api.assert_tx_status(hash, Err(&expected_err));
}

#[test]
fn test_bad_artifact_name() {
    let mut testkit = testkit_with_inc_service();
    let api = testkit.api();

    let bad_artifact = ArtifactId {
        runtime_id: RuntimeIdentifier::Rust as _,
        name: "does-not-exist".to_owned(),
        version: "1.0.0".parse().unwrap(),
    };
    let request = deploy_request(bad_artifact.clone(), testkit.height().next());
    let deploy_confirmation_hash = deploy_confirmation_hash_default(&testkit, &request);
    let hash = deploy_artifact(&api, request);

    testkit.create_block();
    // The deploy request transaction was executed...
    api.exonum_api().assert_tx_success(hash);
    // ... but no confirmation was generated ...
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_hash));
    testkit.create_block();

    let api = testkit.api(); // update the API
                             // .. and no artifact was deployed.
    assert!(!artifact_exists(&api, &bad_artifact.name));
}

#[test]
fn test_bad_runtime_id() {
    let mut testkit = testkit_with_inc_service();
    let api = testkit.api();
    let bad_runtime_id = 10_000;

    let artifact = ArtifactId {
        runtime_id: bad_runtime_id,
        ..IncService.artifact_id()
    };
    let request = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation_hash = deploy_confirmation_hash_default(&testkit, &request);
    let hash = deploy_artifact(&api, request);
    testkit.create_block();

    // The deploy request transaction was executed...
    api.exonum_api().assert_tx_success(hash);
    // ... but no confirmation was generated ...
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_hash));

    testkit.create_block();
    let api = testkit.api(); // update the API
                             // ...and no artifact was deployed.
    assert!(!artifact_exists(&api, &artifact.name));
}

#[test]
fn test_empty_service_instance_name() {
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    let api = testkit.api();
    let artifact = default_artifact();
    let empty_instance_name = "";
    let deadline_height = testkit.height().next();
    let request = start_service_request(artifact, empty_instance_name, deadline_height);
    let hash = start_service(&api, request);
    testkit.create_block();

    let system_api = api.exonum_api();
    let expected_err = ErrorMatch::from_fail(&TxError::InvalidInstanceName)
        .with_description_containing("Service instance name should not be empty")
        .for_service(SUPERVISOR_INSTANCE_ID);
    system_api.assert_tx_status(hash, Err(&expected_err));
}

#[test]
fn test_bad_service_instance_name() {
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    let api = testkit.api();
    let artifact = default_artifact();
    let bad_instance_name = "\u{2764}";
    let deadline_height = testkit.height().next();
    let request = start_service_request(artifact, bad_instance_name, deadline_height);
    let hash = start_service(&api, request);
    testkit.create_block();

    let system_api = api.exonum_api();
    let expected_description =
        "Service instance name (\u{2764}) contains illegal character, use only: a-zA-Z0-9 and one of _-";
    let expected_err = ErrorMatch::from_fail(&TxError::InvalidInstanceName)
        .with_description_containing(expected_description)
        .for_service(SUPERVISOR_INSTANCE_ID);
    system_api.assert_tx_status(hash, Err(&expected_err));
}

#[test]
fn test_start_service_instance_twice() {
    let instance_name = "inc";
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    // Start the first instance
    {
        let api = testkit.api();
        assert!(!service_instance_exists(&api, instance_name));

        let deadline = testkit.height().next();
        let request = start_service_request(default_artifact(), instance_name, deadline);
        let hash = start_service(&api, request);
        testkit.create_block();

        api.exonum_api().assert_tx_success(hash);

        let api = testkit.api(); // Update the API
        assert!(service_instance_exists(&api, instance_name));
    }

    // Try to start another instance with the same name
    {
        let api = testkit.api();

        let deadline = testkit.height().next();
        let request = start_service_request(default_artifact(), instance_name, deadline);
        let hash = start_service(&api, request);
        testkit.create_block();

        let system_api = api.exonum_api();
        let expected_err = ErrorMatch::from_fail(&TxError::InstanceExists)
            .for_service(SUPERVISOR_INSTANCE_ID)
            .with_any_description();
        system_api.assert_tx_status(hash, Err(&expected_err));
    }
}

/// Checks that we can start several service instances in one request.
#[test]
fn test_start_two_services_in_one_request() {
    let instance_name_1 = "inc";
    let instance_name_2 = "inc2";
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    let api = testkit.api();
    assert!(!service_instance_exists(&api, instance_name_1));
    assert!(!service_instance_exists(&api, instance_name_2));

    let artifact = default_artifact();
    let deadline = testkit.height().next();

    let request = ConfigPropose::new(0, deadline)
        .start_service(artifact.clone(), instance_name_1, Vec::default())
        .start_service(artifact.clone(), instance_name_2, Vec::default());

    let hash = start_service(&api, request);
    testkit.create_block();

    api.exonum_api().assert_tx_success(hash);

    let api = testkit.api(); // Update the API
    assert!(service_instance_exists(&api, instance_name_1));
    assert!(service_instance_exists(&api, instance_name_2));
}

#[test]
fn test_restart_node_and_start_service_instance() {
    let mut testkit = TestKitBuilder::validator()
        .with_logger()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .with_rust_service(IncService)
        .create();
    deploy_default(&mut testkit);

    // Stop the node.
    let stopped_testkit = testkit.stop();
    // ...and start it again with the same service factory.
    let runtime = add_available_services(stopped_testkit.rust_runtime());
    let mut testkit = stopped_testkit.resume(vec![runtime]);
    let api = testkit.api();

    // Ensure that the deployed artifact still exists.
    assert!(artifact_exists(&api, &default_artifact().name));

    let instance_name = "test_basics";
    let keypair = crypto::gen_keypair();

    // Start IncService's instance now.
    let instance_id = start_service_instance(&mut testkit, instance_name);
    let api = testkit.api(); // update the API

    // Check that the service instance actually works.
    {
        assert_count_is_not_set(&api, instance_name);

        api.send(keypair.inc(instance_id, 0));
        testkit.create_block();
        assert_count(&api, instance_name, 1);

        api.send(keypair.inc(instance_id, 1));
        testkit.create_block();
        assert_count(&api, instance_name, 2);
    }

    // Restart the node again.
    let stopped_testkit = testkit.stop();
    let runtime = add_available_services(stopped_testkit.rust_runtime());
    let mut testkit = stopped_testkit.resume(vec![runtime]);
    let api = testkit.api();

    // Ensure that the started service instance still exists.
    assert!(service_instance_exists(&api, instance_name));

    // Check that the service instance still works.
    {
        assert_count(&api, instance_name, 2);
        api.send(keypair.inc(instance_id, 2));
        testkit.create_block();
        assert_count(&api, instance_name, 3);
    }
}

#[test]
fn test_restart_node_during_artifact_deployment_with_two_validators() {
    let mut testkit = testkit_with_inc_service_and_two_validators();
    let artifact = default_artifact();
    let api = testkit.api();
    assert!(!artifact_exists(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next().next());
    let deploy_confirmation_0 = deploy_confirmation(&testkit, &request_deploy, ValidatorId(0));
    let deploy_confirmation_1 = deploy_confirmation(&testkit, &request_deploy, ValidatorId(1));

    // Send an artifact deploy request from this validator.
    let deploy_artifact_0_tx_hash = deploy_artifact(&api, request_deploy.clone());
    // Emulate an artifact deploy request from the second validator.
    let deploy_artifact_1_tx_hash =
        deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(1));

    testkit.create_block();
    api.exonum_api()
        .assert_txs_success(&[deploy_artifact_0_tx_hash, deploy_artifact_1_tx_hash]);

    // Confirmation is ready.
    assert!(testkit.is_tx_in_pool(&deploy_confirmation_0.object_hash()));
    testkit.create_block();

    // Restart the node again after the first block was created.
    let testkit = testkit.stop();
    let runtime = add_available_services(testkit.rust_runtime());
    let mut testkit = testkit.resume(vec![runtime]);

    // Emulate a confirmation from the second validator.
    testkit.add_tx(deploy_confirmation_1.clone());
    assert!(testkit.is_tx_in_pool(&deploy_confirmation_1.object_hash()));
    testkit.create_block();
    // Both confirmations are gone now.
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_0.object_hash()));
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_1.object_hash()));

    let api = testkit.api(); // update the API
    assert!(artifact_exists(&api, &artifact.name));
}

/// This test emulates a normal workflow with two validators.
#[test]
fn test_two_validators() {
    let mut testkit = testkit_with_inc_service_and_two_validators();
    let artifact = default_artifact();
    let api = testkit.api();
    assert!(!artifact_exists(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation_0 = deploy_confirmation(&testkit, &request_deploy, ValidatorId(0));
    let deploy_confirmation_1 = deploy_confirmation(&testkit, &request_deploy, ValidatorId(1));

    // Send an artifact deploy request from this validator.
    let deploy_artifact_0_tx_hash = deploy_artifact(&api, request_deploy.clone());

    // Emulate an artifact deploy request from the second validator.
    let deploy_artifact_1_tx_hash =
        deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(1));

    testkit.create_block();
    api.exonum_api()
        .assert_txs_success(&[deploy_artifact_0_tx_hash, deploy_artifact_1_tx_hash]);

    // Emulate a confirmation from the second validator.
    testkit.add_tx(deploy_confirmation_1.clone());

    // Both confirmations are ready.
    assert!(testkit.is_tx_in_pool(&deploy_confirmation_0.object_hash()));
    assert!(testkit.is_tx_in_pool(&deploy_confirmation_1.object_hash()));
    testkit.create_block();

    // Both confirmations are gone now.
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_0.object_hash()));
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_1.object_hash()));

    let api = testkit.api(); // update the API
    assert!(artifact_exists(&api, &artifact.name));
    let instance_name = "inc";

    // Start the service now
    {
        assert!(!service_instance_exists(&api, instance_name));
        // Add two heights to the deadline: one for block with config proposal and one for confirmation.
        let deadline = testkit.height().next().next();
        let request_start = start_service_request(default_artifact(), instance_name, deadline);
        let propose_hash = request_start.object_hash();

        // Send a start instance request from this node.
        start_service(&api, request_start.clone());
        testkit.create_block();

        // Confirm changes.
        let signed_txs = build_confirmation_transactions(&testkit, propose_hash, ValidatorId(0));
        testkit
            .create_block_with_transactions(signed_txs)
            .transactions[0]
            .status()
            .expect("Transaction with confirmations discarded.");

        let api = testkit.api(); // Update the API
        assert!(service_instance_exists(&api, instance_name));
    }

    let api = testkit.api(); // Update the API
    let instance_id = find_instance_id(&api, instance_name);
    // Basic check that service works.
    {
        assert_count_is_not_set(&api, instance_name);
        let keypair = crypto::gen_keypair();
        api.send(keypair.inc(instance_id, 0));
        testkit.create_block();
        assert_count(&api, instance_name, 1);

        api.send(keypair.inc(instance_id, 1));
        testkit.create_block();
        assert_count(&api, instance_name, 2);
    }
}

/// This test emulates the case when the second validator doesn't send DeployRequest.
#[test]
fn test_multiple_validators_no_confirmation() {
    let mut testkit = testkit_with_inc_service_and_two_validators();

    let artifact = default_artifact();
    let api = testkit.api();
    assert!(!artifact_exists(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation_0 = deploy_confirmation(&testkit, &request_deploy, ValidatorId(0));

    // Send an artifact deploy request from this validator.
    let deploy_artifact_0_tx_hash = deploy_artifact(&api, request_deploy.clone());

    // Deliberately not sending an artifact deploy request from the second validator.
    testkit.create_block();
    api.exonum_api()
        .assert_tx_success(deploy_artifact_0_tx_hash);

    // Deliberately not sending a confirmation from the second validator.

    // No confirmation was generated ...
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_0.object_hash()));
    testkit.create_block();
    // ...and no artifact was deployed.
    assert!(!artifact_exists(&testkit.api(), &artifact.name));
}

// Test that auditor can't send any requests.
#[test]
fn test_auditor_cant_send_requests() {
    let mut testkit = testkit_with_inc_service_auditor_validator();

    let artifact = default_artifact();
    let api = testkit.api();

    assert!(!artifact_exists(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());

    // Try to send an artifact deploy request from the auditor.
    let deploy_request_from_auditor = {
        // Manually signing the tx with auditor's keypair.
        let confirmation: DeployConfirmation = request_deploy.clone().into();
        testkit
            .us()
            .service_keypair()
            .confirm_artifact_deploy(SUPERVISOR_INSTANCE_ID, confirmation)
    };
    testkit.add_tx(deploy_request_from_auditor.clone());

    // Emulate an artifact deploy request from the second validator.
    let deploy_artifact_validator_tx_hash =
        deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(0));

    testkit.create_block();

    let system_api = api.exonum_api();

    // Emulated request executed as fine...
    system_api.assert_tx_success(deploy_artifact_validator_tx_hash);

    // ... but the auditor's request is failed as expected.
    let expected_err =
        ErrorMatch::from_fail(&TxError::UnknownAuthor).for_service(SUPERVISOR_INSTANCE_ID);
    system_api.assert_tx_status(
        deploy_request_from_auditor.object_hash(),
        Err(&expected_err),
    );
}

/// This test emulates a normal workflow with a validator and an auditor.
#[test]
fn test_auditor_normal_workflow() {
    let mut testkit = testkit_with_inc_service_auditor_validator();
    let artifact = default_artifact();
    let api = testkit.api();
    assert!(!artifact_exists(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation = deploy_confirmation(&testkit, &request_deploy, ValidatorId(0));

    // Emulate an artifact deploy request from the validator.
    let deploy_artifact_tx_hash =
        deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(0));
    testkit.create_block();
    api.exonum_api().assert_tx_success(deploy_artifact_tx_hash);

    // Emulate a confirmation from the validator.
    testkit.add_tx(deploy_confirmation.clone());
    // The confirmation is in the pool.
    assert!(testkit.is_tx_in_pool(&deploy_confirmation.object_hash()));
    testkit.create_block();

    // The confirmation is gone.
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation.object_hash()));
    // The artifact is deployed.
    assert!(artifact_exists(&testkit.api(), &artifact.name));

    let instance_name = "inc";
    // Start the service now
    {
        let api = testkit.api();
        assert!(!service_instance_exists(&api, instance_name));
        let deadline = testkit.height().next();
        let request_start = start_service_request(default_artifact(), instance_name, deadline);

        // Emulate a start instance request from the validator.
        let start_service_tx_hash =
            start_service_manually(&mut testkit, &request_start, ValidatorId(0));
        testkit.create_block();
        api.exonum_api().assert_tx_success(start_service_tx_hash);
        let api = testkit.api(); // Update the API
        assert!(service_instance_exists(&api, instance_name));
    }

    let api = testkit.api(); // Update the API
    let instance_id = find_instance_id(&api, instance_name);

    // Check that service still works.
    {
        assert_count_is_not_set(&api, instance_name);
        let keypair = crypto::gen_keypair();
        api.send(keypair.inc(instance_id, 0));
        testkit.create_block();
        assert_count(&api, instance_name, 1);
        api.send(keypair.inc(instance_id, 1));
        testkit.create_block();
        assert_count(&api, instance_name, 2);
    }
}

/// This test emulates a deploy confirmation with 12 validators.
/// Here we send confirmations by every validator and expect deploy to start.
#[test]
fn test_multiple_validators_deploy_confirm() {
    let validators_count = 12;
    let mut testkit = testkit_with_inc_service_and_n_validators(validators_count);
    let artifact = default_artifact();
    let api = testkit.api();
    assert!(!artifact_exists(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());

    // Send deploy requests by every validator.
    let deploy_tx_hashes: Vec<exonum_crypto::Hash> = (0..validators_count)
        .map(|i| deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(i)))
        .collect();

    // Verify that every transaction succeeded (even for confirmations
    // sent after the quorum was achieved).
    testkit.create_block();
    api.exonum_api().assert_txs_success(&deploy_tx_hashes);

    // Send deploy confirmations by every validator.
    let deploy_confirmations: Vec<Verified<AnyTx>> = (0..validators_count)
        .map(|i| deploy_confirmation(&testkit, &request_deploy, ValidatorId(i)))
        .collect();

    testkit.create_block_with_transactions(deploy_confirmations);

    // Check that artifact is deployed now.
    let api = testkit.api(); // update the API
    assert!(artifact_exists(&api, &artifact.name));
}

/// This test emulates a deploy confirmation with 12 validators.
/// Here we send confirmations by the byzantine majority (2/3+1) validators
/// and expect deploy to start.
#[test]
fn test_multiple_validators_deploy_confirm_byzantine_majority() {
    let validators_count = 12;
    let byzantine_majority = (validators_count * 2 / 3) + 1;
    let mut testkit = testkit_with_inc_service_and_n_validators(validators_count);
    let artifact = default_artifact();
    let api = testkit.api();
    assert!(!artifact_exists(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());

    // Send deploy requests by byzantine majority of validators.
    let deploy_tx_hashes: Vec<exonum_crypto::Hash> = (0..byzantine_majority)
        .map(|i| deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(i)))
        .collect();

    testkit.create_block();
    api.exonum_api().assert_txs_success(&deploy_tx_hashes);

    // Send deploy confirmations by every validator.
    let deploy_confirmations: Vec<Verified<AnyTx>> = (0..validators_count)
        .map(|i| deploy_confirmation(&testkit, &request_deploy, ValidatorId(i)))
        .collect();

    testkit.create_block_with_transactions(deploy_confirmations);

    // Check that artifact is deployed now.
    let api = testkit.api(); // update the API
    assert!(artifact_exists(&api, &artifact.name));
}

/// This test emulates a deploy confirmation with 12 validators.
/// Here we send confirmations by the byzantine minority (2/3) validators
/// and expect deploy to not start.
#[test]
fn test_multiple_validators_deploy_confirm_byzantine_minority() {
    let validators_count = 12;
    let byzantine_minority = validators_count * 2 / 3;
    let mut testkit = testkit_with_inc_service_and_n_validators(validators_count);
    let artifact = default_artifact();
    let api = testkit.api();
    assert!(!artifact_exists(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());

    // Send deploy requests by byzantine majority of validators.
    let deploy_tx_hashes: Vec<_> = (0..byzantine_minority)
        .map(|i| deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(i)))
        .collect();
    testkit.create_block();
    api.exonum_api().assert_txs_success(&deploy_tx_hashes);

    // Try to send confirmation. It should fail, since deploy was not approved
    // and thus not registered.
    let confirmation = deploy_confirmation(&testkit, &request_deploy, ValidatorId(0));
    testkit.add_tx(confirmation.clone());
    testkit.create_block();

    let expected_err = ErrorMatch::from_fail(&TxError::DeployRequestNotRegistered)
        .for_service(SUPERVISOR_INSTANCE_ID);
    api.exonum_api()
        .assert_tx_status(confirmation.object_hash(), Err(&expected_err));
}

/// Checks that service IDs are assigned sequentially starting from the
/// ID next to max builtin ID.
#[test]
fn test_id_assignment() {
    let max_builtin_id = SUPERVISOR_INSTANCE_ID;

    // Deploy inc service & start two instances.
    let instance_name_1 = "inc";
    let instance_name_2 = "inc2";
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    let artifact = default_artifact();
    let deadline = testkit.height().next();

    let request = ConfigPropose::new(0, deadline)
        .start_service(artifact.clone(), instance_name_1, Vec::default())
        .start_service(artifact.clone(), instance_name_2, Vec::default());

    let api = testkit.api();
    start_service(&api, request);
    testkit.create_block();

    // Check that new instances have IDs 1 and 2.
    let api = testkit.api();
    assert_eq!(find_instance_id(&api, instance_name_1), max_builtin_id + 1);
    assert_eq!(find_instance_id(&api, instance_name_2), max_builtin_id + 2);
}

/// Checks that if builtin IDs space is sparse (here we have `Supervisor` with ID 0 and
/// `IncService` with ID 100), the ID for the new service will be next to the max
/// builtin ID (101 in our case).
#[test]
fn test_id_assignment_sparse() {
    let max_builtin_id = 100;
    let inc_service = IncService;
    let inc_service_artifact = inc_service.artifact_id();

    // Create testkit with builtin instance with ID 100.
    let mut testkit = TestKitBuilder::validator()
        .with_logger()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .with_artifact(inc_service_artifact.clone())
        .with_instance(inc_service_artifact.into_default_instance(max_builtin_id, "inc"))
        .with_rust_service(inc_service)
        .create();

    let artifact = default_artifact();
    let deadline = testkit.height().next();

    let instance_name = "inc2";
    let request = ConfigPropose::new(0, deadline).start_service(
        artifact.clone(),
        instance_name,
        Vec::default(),
    );

    let api = testkit.api();
    start_service(&api, request);
    testkit.create_block();

    // Check that new instance has ID 101.
    let api = testkit.api();
    assert_eq!(find_instance_id(&api, instance_name), max_builtin_id + 1);
}
