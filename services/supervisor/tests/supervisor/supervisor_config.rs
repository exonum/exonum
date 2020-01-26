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

//! Tests for the `Supervisor` service configuration, including
//! `Supervisor` service initialization, using `Configure` interface
//! and API endpoints associated with configuration.

use exonum::runtime::{SnapshotExt, SUPERVISOR_INSTANCE_ID};
use exonum_merkledb::BinaryValue;
use exonum_rust_runtime::ServiceFactory;
use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};

use exonum_supervisor::{
    supervisor_name, ConfigChange, ConfigPropose, Schema, ServiceConfig, Supervisor,
    SupervisorConfig,
};

use crate::{config_api::create_proposal, utils::CFG_CHANGE_HEIGHT};

/// Asserts that current supervisor configuration equals to the provided one.
fn assert_supervisor_config(testkit: &TestKit, config: SupervisorConfig) {
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(supervisor_name()).unwrap();
    let current_config = schema.configuration.get().unwrap();
    assert_eq!(current_config, config);
}

/// Checks that initial configuration providers (`Supervisor::simple_config()` and
/// `Supervisor::decentralized_config()`) provide correct values and Supervisor
/// loads them as expected.
#[test]
fn initial_configuration() {
    // Check for simple mode.
    let testkit = TestKitBuilder::validator()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();
    assert_supervisor_config(&testkit, Supervisor::simple_config());

    // Check for decentralized mode.
    let testkit = TestKitBuilder::validator()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .build();
    assert_supervisor_config(&testkit, Supervisor::decentralized_config());
}

/// Checks that incorrect configuration is not accepted by `Supervisor::initialize`.
#[test]
#[should_panic(expected = "Invalid configuration for supervisor.")]
fn incorrect_configuration() {
    let incorrect_config = vec![0x12, 0x34]; // Obviously incorrect config.
    let incorrect_instance = Supervisor
        .artifact_id()
        .into_default_instance(SUPERVISOR_INSTANCE_ID, Supervisor::NAME)
        .with_constructor(incorrect_config);

    let _testkit = TestKitBuilder::validator()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(incorrect_instance)
        .build();

    // By this moment, genesis block should be created and node is expected to panic.
}

/// Checks that configuration of the supervisor can be changed via `Configure` interface.
#[test]
fn configure_call() {
    let mut testkit = TestKitBuilder::validator()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();

    // Change config to decentralized.
    let configuration_change = ServiceConfig {
        instance_id: SUPERVISOR_INSTANCE_ID,
        params: Supervisor::decentralized_config().into_bytes(),
    };

    // Create proposal.
    let config_proposal = ConfigPropose {
        actual_from: CFG_CHANGE_HEIGHT,
        changes: vec![ConfigChange::Service(configuration_change)],
        configuration_number: 0,
    };

    // Apply it (in simple mode no confirmations required).
    create_proposal(&testkit.api(), config_proposal.clone());
    testkit.create_blocks_until(CFG_CHANGE_HEIGHT.next());

    // Check that supervisor now in the decentralized mode.
    assert_supervisor_config(&testkit, Supervisor::decentralized_config());
}

/// Checks that `supervisor-config` works as expected.
#[test]
fn supervisor_config_api() {
    let mut testkit = TestKitBuilder::validator()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .build();
    assert_eq!(
        testkit
            .api()
            .private(ApiKind::Service("supervisor"))
            .get::<SupervisorConfig>("supervisor-config")
            .unwrap(),
        Supervisor::simple_config(),
    );

    // Check for decentralized mode.
    let mut testkit = TestKitBuilder::validator()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .build();
    assert_eq!(
        testkit
            .api()
            .private(ApiKind::Service("supervisor"))
            .get::<SupervisorConfig>("supervisor-config")
            .unwrap(),
        Supervisor::decentralized_config(),
    );
}
