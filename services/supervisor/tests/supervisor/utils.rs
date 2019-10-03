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

use exonum_merkledb::Entry;
use exonum_testkit::{TestKit, TestKitBuilder};

use exonum::runtime::{InstanceId, ServiceConfig};
use exonum::{
    blockchain::{ConsensusConfig, InstanceCollection},
    crypto::Hash,
    helpers::{Height, ValidatorId},
    messages::{AnyTx, Verified},
    runtime::{rust::Transaction, ConfigChange, SUPERVISOR_INSTANCE_ID, SUPERVISOR_INSTANCE_NAME},
};

use crate::{
    IncService as ConfigChangeService, SERVICE_ID as CONFIG_SERVICE_ID,
    SERVICE_NAME as CONFIG_SERVICE_NAME,
};
use exonum_supervisor::{ConfigPropose, ConfigVote, Schema, Supervisor};

pub const SECOND_SERVICE_ID: InstanceId = 119;
pub const SECOND_SERVICE_NAME: &str = "change-service";

pub fn config_propose_entry(testkit: &TestKit) -> Option<ConfigPropose> {
    let snapshot = testkit.snapshot();
    Schema::new(SUPERVISOR_INSTANCE_NAME, &snapshot)
        .config_propose_with_hash_entry()
        .get()
        .map(|entry| entry.config_propose)
}

pub fn sign_config_propose_transaction(
    testkit: &TestKit,
    config: ConfigPropose,
    initiator_id: ValidatorId,
) -> Verified<AnyTx> {
    let keys = &testkit.validator(initiator_id).service_keypair();
    config.sign(SUPERVISOR_INSTANCE_ID, keys.0, &keys.1)
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
            let keys = validator.service_keypair();
            ConfigVote {
                propose_hash: proposal_hash,
            }
            .sign(SUPERVISOR_INSTANCE_ID, keys.0, &keys.1)
        })
        .collect()
}

pub struct ConfigProposeConstructor {
    config_propose: ConfigPropose,
}

impl ConfigProposeConstructor {
    pub fn new(cfg_change_height: Height) -> Self {
        ConfigProposeConstructor {
            config_propose: ConfigPropose {
                actual_from: cfg_change_height,
                changes: vec![],
            },
        }
    }

    pub fn extend_consensus_config_propose(mut self, consensus_config: ConsensusConfig) -> Self {
        self.config_propose
            .changes
            .push(ConfigChange::Consensus(consensus_config));
        self
    }

    pub fn extend_service_config_propose(mut self, param: String) -> Self {
        self.config_propose
            .changes
            .push(ConfigChange::Service(ServiceConfig {
                instance_id: CONFIG_SERVICE_ID,
                params: param.into_bytes(),
            }));
        self
    }

    pub fn extend_second_service_config_propose(mut self, param: String) -> Self {
        self.config_propose
            .changes
            .push(ConfigChange::Service(ServiceConfig {
                instance_id: SECOND_SERVICE_ID,
                params: param.into_bytes(),
            }));
        self
    }

    pub fn config_propose(&self) -> ConfigPropose {
        self.config_propose.clone()
    }
}

pub fn consensus_config_propose_first_variant(testkit: &TestKit) -> ConsensusConfig {
    let mut cfg = testkit.consensus_config();
    // Change any config field.
    // For test purpose ut doesn't matter what exactly filed will be changed.
    cfg.min_propose_timeout = 20;
    cfg
}

pub fn consensus_config_propose_second_variant(testkit: &TestKit) -> ConsensusConfig {
    let mut cfg = testkit.consensus_config();
    // Change any config field.
    // For test purpose ut doesn't matter what exactly filed will be changed.
    cfg.min_propose_timeout = 30;
    cfg
}

pub fn testkit_with_change_service_and_static_instance(validator_count: u16) -> TestKit {
    let service = ConfigChangeService;
    let collection =
        InstanceCollection::new(service).with_instance(CONFIG_SERVICE_ID, CONFIG_SERVICE_NAME, ());
    TestKitBuilder::validator()
        .with_validators(validator_count)
        .with_service(Supervisor)
        .with_service(collection)
        .create()
}

pub fn testkit_with_two_change_service_and_static_instance(validator_count: u16) -> TestKit {
    let service = ConfigChangeService;
    let collection = InstanceCollection::new(service)
        .with_instance(CONFIG_SERVICE_ID, CONFIG_SERVICE_NAME, ())
        .with_instance(SECOND_SERVICE_ID, SECOND_SERVICE_NAME, ());
    TestKitBuilder::validator()
        .with_validators(validator_count)
        .with_service(Supervisor)
        .with_service(collection)
        .create()
}

pub fn check_service_actual_param(testkit: &TestKit, param: Option<String>) {
    let actual_params: Option<String> = Entry::new(
        format!("{}.params", CONFIG_SERVICE_NAME),
        &testkit.snapshot(),
    )
    .get();

    assert_eq!(actual_params, param);
}

pub fn check_second_service_actual_param(testkit: &TestKit, param: Option<String>) {
    let actual_params: Option<String> = Entry::new(
        format!("{}.params", SECOND_SERVICE_NAME),
        &testkit.snapshot(),
    )
    .get();

    assert_eq!(actual_params, param);
}
