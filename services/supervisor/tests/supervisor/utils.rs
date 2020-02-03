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
    blockchain::ConsensusConfig,
    crypto::Hash,
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Verified},
    runtime::{InstanceId, SnapshotExt, SUPERVISOR_INSTANCE_ID},
};
use exonum_merkledb::access::AccessExt;
use exonum_rust_runtime::ServiceFactory;
use exonum_testkit::{TestKit, TestKitBuilder};

use crate::{
    IncService as ConfigChangeService, SERVICE_ID as CONFIG_SERVICE_ID,
    SERVICE_NAME as CONFIG_SERVICE_NAME,
};
use exonum_supervisor::{
    supervisor_name, ConfigChange, ConfigPropose, ConfigVote, Schema, ServiceConfig, Supervisor,
    SupervisorInterface,
};

pub const CFG_CHANGE_HEIGHT: Height = Height(3);

pub const SECOND_SERVICE_ID: InstanceId = 119;
pub const SECOND_SERVICE_NAME: &str = "change-service";

pub fn config_propose_entry(testkit: &TestKit) -> Option<ConfigPropose> {
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(supervisor_name()).unwrap();
    schema
        .pending_proposal
        .get()
        .map(|entry| entry.config_propose)
}

pub fn sign_config_propose_transaction(
    testkit: &TestKit,
    config: ConfigPropose,
    initiator_id: ValidatorId,
) -> Verified<AnyTx> {
    let keys = testkit.validator(initiator_id).service_keypair();
    keys.propose_config_change(SUPERVISOR_INSTANCE_ID, config)
}

pub fn build_confirmation_transactions(
    testkit: &TestKit,
    proposal_hash: Hash,
    initiator_id: ValidatorId,
) -> Vec<Verified<AnyTx>> {
    testkit
        .network()
        .validators()
        .iter()
        .filter(|validator| validator.validator_id() != Some(initiator_id))
        .map(|validator| {
            validator.service_keypair().confirm_config_change(
                SUPERVISOR_INSTANCE_ID,
                ConfigVote {
                    propose_hash: proposal_hash,
                },
            )
        })
        .collect()
}

pub struct ConfigProposeBuilder {
    config_propose: ConfigPropose,
}

impl ConfigProposeBuilder {
    pub fn new(cfg_change_height: Height) -> Self {
        ConfigProposeBuilder {
            config_propose: ConfigPropose {
                actual_from: cfg_change_height,
                changes: vec![],
                // As in the common cases we test only one config, it's ok
                // to have default value of 0 for test purposes.
                configuration_number: 0,
            },
        }
    }

    pub fn configuration_number(mut self, configuration_number: u64) -> Self {
        self.config_propose.configuration_number = configuration_number;
        self
    }

    pub fn extend_consensus_config_propose(mut self, consensus_config: ConsensusConfig) -> Self {
        self.config_propose
            .changes
            .push(ConfigChange::Consensus(consensus_config));
        self
    }

    pub fn extend_service_config_propose(mut self, params: String) -> Self {
        self.config_propose
            .changes
            .push(ConfigChange::Service(ServiceConfig {
                instance_id: CONFIG_SERVICE_ID,
                params: params.into_bytes(),
            }));
        self
    }

    pub fn extend_second_service_config_propose(mut self, params: String) -> Self {
        self.config_propose
            .changes
            .push(ConfigChange::Service(ServiceConfig {
                instance_id: SECOND_SERVICE_ID,
                params: params.into_bytes(),
            }));
        self
    }

    pub fn build(&self) -> ConfigPropose {
        self.config_propose.clone()
    }
}

pub fn consensus_config_propose_first_variant(testkit: &TestKit) -> ConsensusConfig {
    let mut cfg = testkit.consensus_config();
    // Change any config field.
    // For test purpose it doesn't matter what exactly filed will be changed.
    cfg.min_propose_timeout += 1;
    cfg
}

pub fn consensus_config_propose_second_variant(testkit: &TestKit) -> ConsensusConfig {
    let mut cfg = testkit.consensus_config();
    // Change any config field.
    // For test purpose it doesn't matter what exactly filed will be changed.
    cfg.min_propose_timeout += 2;
    cfg
}

pub fn testkit_with_supervisor(validator_count: u16) -> TestKit {
    TestKitBuilder::validator()
        .with_logger()
        .with_validators(validator_count)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .build()
}

pub fn testkit_with_supervisor_and_service(validator_count: u16) -> TestKit {
    TestKitBuilder::validator()
        .with_validators(validator_count)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .with_default_rust_service(ConfigChangeService)
        .build()
}

pub fn testkit_with_supervisor_and_2_services(validator_count: u16) -> TestKit {
    let service = ConfigChangeService;
    let artifact = service.artifact_id();
    TestKitBuilder::validator()
        .with_validators(validator_count)
        .with_rust_service(Supervisor)
        .with_artifact(Supervisor.artifact_id())
        .with_instance(Supervisor::decentralized())
        .with_artifact(artifact.clone())
        .with_instance(
            artifact
                .clone()
                .into_default_instance(CONFIG_SERVICE_ID, CONFIG_SERVICE_NAME),
        )
        .with_instance(artifact.into_default_instance(SECOND_SERVICE_ID, SECOND_SERVICE_NAME))
        .with_rust_service(service)
        .build()
}

pub fn check_service_actual_param(testkit: &TestKit, param: Option<String>) {
    let snapshot = testkit.snapshot();
    let actual_params = snapshot
        .for_service(CONFIG_SERVICE_NAME)
        .unwrap()
        .get_entry::<_, String>("params");
    match param {
        Some(param) => assert_eq!(actual_params.get().unwrap(), param),
        None => assert!(!actual_params.exists()),
    }
}

pub fn check_second_service_actual_param(testkit: &TestKit, param: Option<String>) {
    let snapshot = testkit.snapshot();
    let actual_params = snapshot
        .for_service(SECOND_SERVICE_NAME)
        .unwrap()
        .get_entry::<_, String>("params");
    match param {
        Some(param) => assert_eq!(actual_params.get().unwrap(), param),
        None => assert!(!actual_params.exists()),
    }
}
