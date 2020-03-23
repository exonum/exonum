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
use exonum_testkit::{Spec, TestKit, TestKitBuilder};

use crate::{
    IncService as ConfigChangeService, SERVICE_ID as CONFIG_SERVICE_ID,
    SERVICE_NAME as CONFIG_SERVICE_NAME,
};
use exonum_supervisor::{
    ConfigChange, ConfigPropose, ConfigVote, Schema, Supervisor, SupervisorInterface,
};

pub const CFG_CHANGE_HEIGHT: Height = Height(3);
pub const SECOND_SERVICE_ID: InstanceId = 119;
pub const SECOND_SERVICE_NAME: &str = "change-service";

pub fn config_propose_entry(testkit: &TestKit) -> Option<ConfigPropose> {
    let snapshot = testkit.snapshot();
    let schema: Schema<_> = snapshot.service_schema(Supervisor::NAME).unwrap();
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
            validator
                .service_keypair()
                .confirm_config_change(SUPERVISOR_INSTANCE_ID, ConfigVote::new(proposal_hash))
        })
        .collect()
}

pub struct ConfigProposeBuilder {
    config_propose: ConfigPropose,
}

impl ConfigProposeBuilder {
    pub fn new(cfg_change_height: Height) -> Self {
        ConfigProposeBuilder {
            config_propose: ConfigPropose::new(0, cfg_change_height),
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
        self.config_propose = self
            .config_propose
            .service_config(CONFIG_SERVICE_ID, params);
        self
    }

    pub fn extend_second_service_config_propose(mut self, params: String) -> Self {
        self.config_propose = self
            .config_propose
            .service_config(SECOND_SERVICE_ID, params);
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
        .with(Supervisor::decentralized())
        .build()
}

pub fn testkit_with_supervisor_and_service(validator_count: u16) -> TestKit {
    TestKitBuilder::validator()
        .with_validators(validator_count)
        .with(Supervisor::decentralized())
        .with(Spec::new(ConfigChangeService).with_default_instance())
        .build()
}

pub fn testkit_with_supervisor_and_2_services(validator_count: u16) -> TestKit {
    let services = Spec::new(ConfigChangeService)
        .with_instance(CONFIG_SERVICE_ID, CONFIG_SERVICE_NAME, ())
        .with_instance(SECOND_SERVICE_ID, SECOND_SERVICE_NAME, ());
    TestKitBuilder::validator()
        .with_validators(validator_count)
        .with(Supervisor::decentralized())
        .with(services)
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
