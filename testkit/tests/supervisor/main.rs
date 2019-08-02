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

use exonum_merkledb::ObjectHash;
use exonum_testkit::{ApiKind, InstanceCollection, TestKit, TestKitApi, TestKitBuilder};
use serde_json::json;

use exonum::{
    api, crypto,
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Verified},
    proto::Any,
    runtime::{
        rust::{service::ServiceFactory, Transaction},
        supervisor::{DeployConfirmation, DeployRequest, StartService, Supervisor},
        ArtifactId, RuntimeIdentifier, ServiceInstanceId,
    },
};

use crate::inc::{IncService, TxInc, SERVICE_ID, SERVICE_NAME};

mod inc;
mod proto;

fn artifact_default() -> ArtifactId {
    ArtifactId {
        runtime_id: RuntimeIdentifier::Rust as _,
        name: IncService::new().artifact_id().to_string(),
    }
}

fn assert_count(api: &TestKitApi, service_name: &'static str, expected_count: u64) {
    let real_count: u64 = api
        .public(ApiKind::Service(service_name))
        .get("v1/counter")
        .unwrap();
    assert_eq!(real_count, expected_count);
}

fn assert_no_count(api: &TestKitApi, service_name: &'static str) {
    let response: api::Result<u64> = api.public(ApiKind::Service(service_name)).get("v1/counter");
    assert!(response.is_err());
}

fn does_artifact_exist(api: &TestKitApi, name: &str) -> bool {
    let artifacts = &api.system_public_api().services().artifacts;
    artifacts.iter().any(|a| a.name == name)
}

fn does_service_instance_exist(api: &TestKitApi, name: &str) -> bool {
    let services = &api.system_public_api().services().services;
    services.iter().any(|s| s.name == name)
}

fn find_instance_id(api: &TestKitApi, instance_name: &str) -> ServiceInstanceId {
    let services = &api.system_public_api().services().services;
    services
        .iter()
        .find(|service| service.name == instance_name)
        .expect("Can't find the instance")
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
    let service_id = Supervisor::BUILTIN_ID;
    let keys = &testkit.validator(validator_id).service_keypair();
    let signed_request = request.clone().sign(service_id, keys.0, &keys.1);
    testkit.add_tx(signed_request.clone());
    signed_request.object_hash()
}

// TODO: Just implement PrivateApi (from exonum/src/runtime/supervisor/api.rs) for TestKitApi.
fn start_service(api: &TestKitApi, request: StartService) -> crypto::Hash {
    let hash: crypto::Hash = api
        .private(ApiKind::Service("supervisor"))
        .query(&request)
        .post("start-service")
        .unwrap();
    hash
}

fn start_service_manually(
    testkit: &mut TestKit,
    request: &StartService,
    validator_id: ValidatorId,
) -> crypto::Hash {
    let service_id = Supervisor::BUILTIN_ID;
    let keys = &testkit.validator(validator_id).service_keypair();
    let signed_request = request.clone().sign(service_id, keys.0, &keys.1);
    testkit.add_tx(signed_request.clone());
    signed_request.object_hash()
}

fn deploy_confirmation(
    testkit: &TestKit,
    request: &DeployRequest,
    validator_id: ValidatorId,
) -> Verified<AnyTx> {
    let service_id = Supervisor::BUILTIN_ID;
    let confirmation: DeployConfirmation = request.clone().into();
    let keys = &testkit.validator(validator_id).service_keypair();
    confirmation.sign(service_id, keys.0, &keys.1)
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
        spec: Any::default(),
        deadline_height,
    }
}

fn start_service_request(
    artifact: ArtifactId,
    name: impl Into<String>,
    deadline_height: Height,
) -> StartService {
    StartService {
        artifact,
        name: name.into(),
        config: Any::default(),
        deadline_height,
    }
}

fn deploy_default(testkit: &mut TestKit) {
    let artifact = artifact_default();
    let api = testkit.api();

    assert!(!does_artifact_exist(&api, &artifact.name));

    let request = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation_hash = deploy_confirmation_hash_default(testkit, &request);
    let hash = deploy_artifact(&api, request);
    testkit.create_block();

    api.system_public_api().assert_tx_success(hash);

    // Confirmation is ready.
    assert!(testkit.is_tx_in_pool(&deploy_confirmation_hash));

    testkit.create_block();

    // Confirmation is gone now.
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_hash));

    let api = testkit.api(); // update the API
    assert!(does_artifact_exist(&api, &artifact.name));
}

fn start_service_instance(testkit: &mut TestKit, instance_name: &str) -> ServiceInstanceId {
    let api = testkit.api();

    assert!(!does_service_instance_exist(&api, instance_name));

    let request = start_service_request(artifact_default(), instance_name, testkit.height().next());
    let hash = start_service(&api, request);
    testkit.create_block();

    api.system_public_api().assert_tx_success(hash);

    let api = testkit.api(); // Update the API
    assert!(does_service_instance_exist(&api, instance_name));
    find_instance_id(&api, instance_name)
}

fn testkit_with_inc_service() -> TestKit {
    let service = IncService::new();
    TestKitBuilder::validator()
        .with_logger()
        .with_service(InstanceCollection::new(service))
        .create()
}

fn testkit_with_inc_service_and_two_validators() -> TestKit {
    let service = IncService::new();
    TestKitBuilder::validator()
        .with_logger()
        .with_service(InstanceCollection::new(service))
        .with_validators(2)
        .create()
}

fn testkit_with_inc_service_auditor_validator() -> TestKit {
    let service = IncService::new();
    TestKitBuilder::auditor()
        .with_logger()
        .with_service(InstanceCollection::new(service))
        .with_validators(1)
        .create()
}

fn testkit_with_inc_service_and_static_instance() -> TestKit {
    let service = IncService::new();
    let collection = InstanceCollection::new(service).with_instance(SERVICE_ID, SERVICE_NAME, ());
    TestKitBuilder::validator()
        .with_logger()
        .with_service(collection)
        .create()
}

/// Just test that the Inc service works as intended.
#[test]
fn test_static_service() {
    let mut testkit = testkit_with_inc_service_and_static_instance();
    let api = testkit.api();

    assert_no_count(&api, SERVICE_NAME);

    let (key_pub, key_priv) = crypto::gen_keypair();

    api.send(TxInc { seed: 0 }.sign(SERVICE_ID, key_pub, &key_priv));
    testkit.create_block();
    assert_count(&api, SERVICE_NAME, 1);

    api.send(TxInc { seed: 1 }.sign(SERVICE_ID, key_pub, &key_priv));
    testkit.create_block();
    assert_count(&api, SERVICE_NAME, 2);
}

/// Tests a normal dynamic service workflow with one validator.
#[test]
fn test_dynamic_service_nowrmal_workflow() {
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);
    let instance_name = "test_basics";
    let instance_id = start_service_instance(&mut testkit, instance_name);
    let api = testkit.api();

    assert_no_count(&api, instance_name);

    let (key_pub, key_priv) = crypto::gen_keypair();

    api.send(TxInc { seed: 0 }.sign(instance_id, key_pub, &key_priv));
    testkit.create_block();
    assert_count(&api, instance_name, 1);

    api.send(TxInc { seed: 1 }.sign(instance_id, key_pub, &key_priv));
    testkit.create_block();
    assert_count(&api, instance_name, 2);
}

#[test]
fn test_artifact_deploy_bad_deadline_height() {
    let mut testkit = testkit_with_inc_service();

    // We skip to Height(1) ...
    testkit.create_block();

    // ... but set Height(0) as a deadline.
    let bad_deadline_height = testkit.height().previous();

    let artifact = artifact_default();
    let api = testkit.api();

    let request = deploy_request(artifact.clone(), bad_deadline_height);
    let deploy_confirmation_hash = deploy_confirmation_hash_default(&testkit, &request);
    let hash = deploy_artifact(&api, request);
    testkit.create_block();

    assert!(!does_artifact_exist(&api, &artifact.name));

    // No confirmation was generated
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_hash));

    let system_api = api.system_public_api();
    let expected_status = json!({
       "type": "service_error",
       "code": 2,
       "description": "Deadline exceeded for the current transaction.",
    });
    system_api.assert_tx_status(hash, &expected_status);
}

#[test]
fn test_start_service_instance_bad_deadline_height() {
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    let api = testkit.api();
    let artifact = artifact_default();
    let instance_name = "inc_test";
    let bad_deadline_height = testkit.height().previous();
    let request = start_service_request(artifact, instance_name, bad_deadline_height);
    let hash = start_service(&api, request);
    testkit.create_block();

    let system_api = api.system_public_api();
    let expected_status = json!({
       "type": "service_error",
       "code": 2,
       "description": "Deadline exceeded for the current transaction.",
    });
    system_api.assert_tx_status(hash, &expected_status);
}

#[test]
fn test_try_run_unregistered_service_instance() {
    let mut testkit = testkit_with_inc_service();
    let api = testkit.api();

    // Deliberately missing the DeployRequest step.

    let instance_name = "wont_run";
    let request = StartService {
        artifact: artifact_default(),
        name: instance_name.into(),
        config: Any::default(),
        deadline_height: Height(1000),
    };
    let hash = start_service(&api, request);
    testkit.create_block();

    let system_api = api.system_public_api();
    let expected_status = json!({
       "type": "dispatcher_error",
       "code": 3,
       "description": "Artifact with the given identifier is not deployed.",
    });
    system_api.assert_tx_status(hash, &expected_status);
}

#[test]
fn test_bad_artifact_name() {
    let mut testkit = testkit_with_inc_service();
    let api = testkit.api();

    let bad_artifact = ArtifactId {
        runtime_id: RuntimeIdentifier::Rust as _,
        name: "does_not_exist/1.0.0".into(),
    };
    let request = deploy_request(bad_artifact.clone(), testkit.height().next());
    let deploy_confirmation_hash = deploy_confirmation_hash_default(&testkit, &request);
    let hash = deploy_artifact(&api, request);

    testkit.create_block();

    // The deploy request transaction was executed...
    api.system_public_api().assert_tx_success(hash);

    // ... but no confirmation was generated ...
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_hash));

    testkit.create_block();

    let api = testkit.api(); // update the API

    // .. and no artifact was deployed.
    assert!(!does_artifact_exist(&api, &bad_artifact.name));
}

#[test]
fn test_bad_runtime_id() {
    let mut testkit = testkit_with_inc_service();
    let api = testkit.api();

    let bad_runtime_id = 10_000;

    let artifact = ArtifactId {
        runtime_id: bad_runtime_id,
        name: IncService::new().artifact_id().to_string(),
    };
    let request = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation_hash = deploy_confirmation_hash_default(&testkit, &request);
    let hash = deploy_artifact(&api, request);

    testkit.create_block();

    // The deploy request transaction was executed...
    api.system_public_api().assert_tx_success(hash);

    // ... but no confirmation was generated ...
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_hash));

    testkit.create_block();

    let api = testkit.api(); // update the API

    // .. and no artifact was deployed.
    assert!(!does_artifact_exist(&api, &artifact.name));
}

#[test]
fn test_empty_service_instance_name() {
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    let api = testkit.api();
    let artifact = artifact_default();
    let empty_instance_name = "";
    let deadline_height = testkit.height().next();
    let request = start_service_request(artifact, empty_instance_name, deadline_height);
    let hash = start_service(&api, request);
    testkit.create_block();

    let system_api = api.system_public_api();
    let expected_status = json!({
       "type": "service_error",
       "code": 7,
       "description": "Service instance name should not be empty",
    });
    system_api.assert_tx_status(hash, &expected_status);
}

#[test]
fn test_bad_service_instance_name() {
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    let api = testkit.api();
    let artifact = artifact_default();
    let bad_instance_name = "\u{2764}";
    let deadline_height = testkit.height().next();
    let request = start_service_request(artifact, bad_instance_name, deadline_height);
    let hash = start_service(&api, request);
    testkit.create_block();

    let system_api = api.system_public_api();
    let expected_status = json!({
       "type": "service_error",
       "code": 7,
       "description": "Service instance name contains illegal character, use only: a-zA-Z0-9 and one of _-.",
    });
    system_api.assert_tx_status(hash, &expected_status);
}

#[test]
fn test_start_service_instance_twice() {
    let instance_name = "inc";
    let mut testkit = testkit_with_inc_service();
    deploy_default(&mut testkit);

    // Start the first instance
    {
        let api = testkit.api();
        assert!(!does_service_instance_exist(&api, instance_name));

        let deadline = testkit.height().next();
        let request = start_service_request(artifact_default(), instance_name, deadline);
        let hash = start_service(&api, request);
        testkit.create_block();

        api.system_public_api().assert_tx_success(hash);

        let api = testkit.api(); // Update the API
        assert!(does_service_instance_exist(&api, instance_name));
    }

    // Try to start another instance with the same name
    {
        let api = testkit.api();

        let deadline = testkit.height().next();
        let request = start_service_request(artifact_default(), instance_name, deadline);
        let hash = start_service(&api, request);
        testkit.create_block();

        let system_api = api.system_public_api();
        let expected_status = json!({
           "type": "service_error",
           "code": 3,
           "description": "Instance with the given name already exists.",
        });
        system_api.assert_tx_status(hash, &expected_status);
    }
}

#[test]
#[should_panic] // TODO: This test shouldn't panic, it's a bug.
fn test_restart_node_and_start_service_instance() {
    let service = IncService::new();
    let mut testkit = TestKitBuilder::validator()
        .with_logger()
        .with_service(InstanceCollection::new(service.clone()))
        .create();

    deploy_default(&mut testkit);

    // TODO: testkit crashes here at the moment. This looks like a bug.
    // thread 'test_restart_node_and_start_service_instance' panicked at 'cannot retrieve database state: CheckpointDbHandler { inner: RwLock { data: CheckpointDbInner { db: TemporaryDB { inner: RocksDB, _dir: TempDir { path: "/tmp/.tmpaTHDzU" } }, backup_stack: [] } } }', src/libcore/result.rs:999:5
    let _testkit_stopped = testkit.stop();

    // TODO: Try to start IncService's instance now
    // let _testkit = testkit_stopped.resume(vec![service]);
}

/// This test emulates a normal workflow with two validators.
#[test]
fn test_multiple_validators() {
    let mut testkit = testkit_with_inc_service_and_two_validators();

    let artifact = artifact_default();
    let api = testkit.api();

    assert!(!does_artifact_exist(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation_0 = deploy_confirmation(&testkit, &request_deploy, ValidatorId(0));
    let deploy_confirmation_1 = deploy_confirmation(&testkit, &request_deploy, ValidatorId(1));

    // Send an artifact deploy request from this validator.
    let deploy_artifact_0_tx_hash = deploy_artifact(&api, request_deploy.clone());

    // Emulate an artifact deploy request from the second validator.
    let deploy_artifact_1_tx_hash =
        deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(1));

    testkit.create_block();

    api.system_public_api()
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
    assert!(does_artifact_exist(&api, &artifact.name));

    let instance_name = "inc";

    // Start the service now
    {
        let api = testkit.api();

        assert!(!does_service_instance_exist(&api, instance_name));

        let deadline = testkit.height().next();
        let request_start = start_service_request(artifact_default(), instance_name, deadline);

        // Send a start instance request from this node.
        let start_service_0_tx_hash = start_service(&api, request_start.clone());

        // Emulate a start instance request from the second validator.
        let start_service_1_tx_hash =
            start_service_manually(&mut testkit, &request_start, ValidatorId(1));

        testkit.create_block();

        api.system_public_api()
            .assert_txs_success(&[start_service_0_tx_hash, start_service_1_tx_hash]);

        let api = testkit.api(); // Update the API
        assert!(does_service_instance_exist(&api, instance_name));
    }

    let api = testkit.api(); // Update the API
    let instance_id = find_instance_id(&api, instance_name);

    // Basic check that service works.
    {
        assert_no_count(&api, instance_name);

        let (key_pub, key_priv) = crypto::gen_keypair();

        api.send(TxInc { seed: 0 }.sign(instance_id, key_pub, &key_priv));
        testkit.create_block();
        assert_count(&api, instance_name, 1);

        api.send(TxInc { seed: 1 }.sign(instance_id, key_pub, &key_priv));
        testkit.create_block();
        assert_count(&api, instance_name, 2);
    }
}

/// This test emulates the case when the second validator doesn't send DeployRequest.
#[test]
fn test_multiple_validators_no_confirmation() {
    let mut testkit = testkit_with_inc_service_and_two_validators();

    let artifact = artifact_default();
    let api = testkit.api();

    assert!(!does_artifact_exist(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation_0 = deploy_confirmation(&testkit, &request_deploy, ValidatorId(0));

    // Send an artifact deploy request from this validator.
    let deploy_artifact_0_tx_hash = deploy_artifact(&api, request_deploy.clone());

    // Deliberately not sending an artifact deploy request from the second validator.

    testkit.create_block();

    api.system_public_api()
        .assert_tx_success(deploy_artifact_0_tx_hash);

    // Deliberately not sending a confirmation from the second validator.

    // No confirmation was generated ...
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation_0.object_hash()));

    testkit.create_block();

    // .. and no artifact was deployed.
    assert!(!does_artifact_exist(&testkit.api(), &artifact.name));
}

// Test that auditor can't send any requests.
#[test]
fn test_auditor_cant_send_requests() {
    let mut testkit = testkit_with_inc_service_auditor_validator();

    let artifact = artifact_default();
    let api = testkit.api();

    assert!(!does_artifact_exist(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());

    // Try to send an artifact deploy request from the auditor.
    let deploy_request_from_auditor = {
        // Manually signing the tx with audtitor's keypair.
        let service_id = Supervisor::BUILTIN_ID;
        let confirmation: DeployConfirmation = request_deploy.clone().into();
        let keys = &testkit.us().service_keypair();
        confirmation.sign(service_id, keys.0, &keys.1)
    };
    testkit.add_tx(deploy_request_from_auditor.clone());

    // Emulate an artifact deploy request from the second validator.
    let deploy_artifact_validator_tx_hash =
        deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(0));

    testkit.create_block();

    let system_api = api.system_public_api();

    // Emulated request executed as fine...
    system_api.assert_tx_success(deploy_artifact_validator_tx_hash);

    // ... but the auditor's request is failed as expected.
    let expected_status = json!({
       "type": "service_error",
       "code": 1,
       "description": "Transaction author is not a validator.",
    });
    system_api.assert_tx_status(deploy_request_from_auditor.object_hash(), &expected_status);
}

/// This test emulates a normal workflow with a validator and an auditor.
#[test]
#[ignore] // TODO: fix this test: `An attempt to register artifact which is not be deployed`
fn test_auditor_norman_workflow() {
    let mut testkit = testkit_with_inc_service_auditor_validator();

    let artifact = artifact_default();
    let api = testkit.api();

    assert!(!does_artifact_exist(&api, &artifact.name));

    let request_deploy = deploy_request(artifact.clone(), testkit.height().next());
    let deploy_confirmation = deploy_confirmation(&testkit, &request_deploy, ValidatorId(0));

    // Emulate an artifact deploy request from the validator.
    let deploy_artifact_tx_hash =
        deploy_artifact_manually(&mut testkit, &request_deploy, ValidatorId(0));

    testkit.create_block();

    api.system_public_api()
        .assert_tx_success(deploy_artifact_tx_hash);

    // Emulate a confirmation from the validator.
    testkit.add_tx(deploy_confirmation.clone());

    // The confirmation is in the pool.
    assert!(testkit.is_tx_in_pool(&deploy_confirmation.object_hash()));

    testkit.create_block();

    // The confirmation is gone.
    assert!(!testkit.is_tx_in_pool(&deploy_confirmation.object_hash()));

    // The artifact is deployed.
    assert!(does_artifact_exist(&testkit.api(), &artifact.name));
}
