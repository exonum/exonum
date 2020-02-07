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

use exonum::runtime::{CoreError, ErrorMatch, ExecutionError, InstanceId, SUPERVISOR_INSTANCE_ID};
use exonum_rust_runtime::ServiceFactory;
use exonum_supervisor::{ConfigPropose, Supervisor, SupervisorInterface};
use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};
use exonum_time::{MockTimeProvider, TimeServiceFactory};

use std::time::SystemTime;

use exonum_timestamping::{Config, TimestampingService};

const TIME_SERVICE_ID: InstanceId = 102;
const TIME_SERVICE_NAME: &str = "time";
const SERVICE_ID: InstanceId = 103;
const SERVICE_NAME: &str = "timestamping";
const SECOND_TIME_SERVICE_ID: InstanceId = 104;
const SECOND_TIME_SERVICE_NAME: &str = "time2";

fn init_testkit(second_time_service: bool) -> (TestKit, MockTimeProvider) {
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
        );

    let testkit = if second_time_service {
        let time_service = TimeServiceFactory::with_provider(mock_provider.clone());
        let time_service_artifact = time_service.artifact_id();
        testkit
            .with_instance(
                time_service_artifact
                    .into_default_instance(SECOND_TIME_SERVICE_ID, SECOND_TIME_SERVICE_NAME),
            )
            .build()
    } else {
        testkit.build()
    };

    (testkit, mock_provider)
}

/// Creates block with `ConfigPropose` tx and returns `Result` with new
/// configuration or corresponding `ExecutionError`.
fn propose_configuration(testkit: &mut TestKit, config: Config) -> Result<(), ExecutionError> {
    let tx = ConfigPropose::immediate(0).service_config(SERVICE_ID, config.clone());
    let keypair = testkit.network().us().service_keypair();
    let tx = keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, tx);
    let block = testkit.create_block_with_transaction(tx);

    if let Err(e) = block[0].status() {
        return Err(e.clone());
    }

    let new_config: Config = testkit
        .api()
        .public(ApiKind::Service(SERVICE_NAME))
        .get("v1/timestamps/config")
        .expect("Failed to get service configuration");

    assert_eq!(config.time_service_name, new_config.time_service_name);
    Ok(())
}

#[test]
fn test_propose_configuration() {
    let (mut testkit, _) = init_testkit(true);
    let config = Config {
        time_service_name: SECOND_TIME_SERVICE_NAME.to_string(),
    };

    // Propose valid configuration.
    propose_configuration(&mut testkit, config).expect("Configuration proposal failed.");
}

#[test]
fn test_propose_invalid_configuration() {
    let (mut testkit, _) = init_testkit(false);
    let incorrect_names = vec!["", " ", "illegal.illegal", "not_service", SERVICE_NAME];

    for name in incorrect_names {
        let config = Config {
            time_service_name: name.to_string(),
        };

        // Propose configuration with invalid time service name.
        let err = propose_configuration(&mut testkit, config)
            .expect_err("Configuration proposal should fail.");

        let expected_err =
            ErrorMatch::from_fail(&CoreError::IncorrectInstanceId).with_any_description();
        assert_eq!(err, expected_err);
    }
}
