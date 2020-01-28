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

use exonum::runtime::{InstanceId, SnapshotExt};

use exonum_merkledb::access::AccessExt;
use exonum_rust_runtime::ServiceFactory;
use exonum_supervisor::{ConfigPropose, Supervisor};
use exonum_testkit::{TestKit, TestKitBuilder};
use exonum_time::{MockTimeProvider, TimeServiceFactory};

use std::time::SystemTime;

use exonum_timestamping::{Config, TimestampingService};

const TIME_SERVICE_ID: InstanceId = 102;
const TIME_SERVICE_NAME: &str = "time";
const SERVICE_ID: InstanceId = 103;
const SERVICE_NAME: &str = "timestamping";

fn init_testkit() -> (TestKit, MockTimeProvider) {
    let mock_provider = MockTimeProvider::new(SystemTime::now().into());
    let time_service = TimeServiceFactory::with_provider(mock_provider.clone());
    let time_service_artifact = time_service.artifact_id();
    let timestamping = TimestampingService;
    let timestamping_artifact = timestamping.artifact_id();

    let testkit = TestKitBuilder::validator()
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::simple())
        .with_rust_service(time_service)
        .with_rust_service(timestamping)
        .with_artifact(time_service_artifact.clone())
        .with_instance(
            time_service_artifact.into_default_instance(TIME_SERVICE_ID, TIME_SERVICE_NAME),
        )
        .with_artifact(timestamping_artifact.clone())
        .with_instance(
            timestamping_artifact
                .into_default_instance(SERVICE_ID, SERVICE_NAME)
                .with_constructor(Config {
                    time_service_name: TIME_SERVICE_NAME.to_owned(),
                }),
        )
        .build();
    (testkit, mock_provider)
}

/// Creates block with `ConfigPropose` tx and returns current service configuration.
fn propose_configuration(testkit: &mut TestKit, config: Config) -> Config {
    let tx = ConfigPropose::immediate(0).service_config(SERVICE_ID, config);

    let initiator_id = testkit.network().us().validator_id().unwrap();
    let (pub_key, sec_key) = &testkit.validator(initiator_id).service_keypair();
    testkit.create_block_with_transaction(tx.sign_for_supervisor(*pub_key, sec_key));

    testkit
        .snapshot()
        .for_service(SERVICE_NAME)
        .unwrap()
        .get_entry("config")
        .get()
        .unwrap()
}

#[test]
fn test_propose_configuration() {
    let (mut testkit, _) = init_testkit();
    let config = Config {
        time_service_name: "time2".to_string(),
    };

    // Propose valid configuration.
    let new_config = propose_configuration(&mut testkit, config.clone());

    assert_eq!(new_config.time_service_name, config.time_service_name);
}

#[test]
fn test_propose_invalid_configuration() {
    let (mut testkit, _) = init_testkit();
    let orig_time_service_name = "time";
    let config = Config {
        time_service_name: "".to_string(),
    };

    // Propose invalid configuration.
    let new_config = propose_configuration(&mut testkit, config);

    // Check that configuration has not changed.
    assert_eq!(new_config.time_service_name, orig_time_service_name);
}
