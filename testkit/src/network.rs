// Copyright 2018 The Exonum Team
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
    blockchain::{ConsensusConfig, GenesisConfig, StoredConfiguration, ValidatorKeys},
    crypto::{self, CryptoHash}, helpers::{Height, Round, ValidatorId},
    messages::{Precommit, Propose},
};
use serde::{Deserialize, Serialize};
use serde_json;

/// Emulated test network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestNetwork {
    us: TestNode,
    validators: Vec<TestNode>,
}

impl TestNetwork {
    /// Creates a new emulated network.
    pub fn new(validator_count: u16) -> Self {
        Self::with_our_role(Some(ValidatorId(0)), validator_count)
    }

    /// Creates a new emulated network with a specific role of the node
    /// the network will be viewed from.
    pub fn with_our_role(us: Option<ValidatorId>, validator_count: u16) -> Self {
        assert!(
            validator_count > 0,
            "At least one validator should be present in the network."
        );

        let validators = (0..validator_count)
            .map(ValidatorId)
            .map(TestNode::new_validator)
            .collect::<Vec<_>>();

        let us = if let Some(ValidatorId(id)) = us {
            validators[id as usize].clone()
        } else {
            TestNode::new_auditor()
        };
        TestNetwork { validators, us }
    }

    /// Returns the node in the emulated network, from whose perspective the testkit operates.
    pub fn us(&self) -> &TestNode {
        &self.us
    }

    /// Returns a slice of all validators in the network.
    pub fn validators(&self) -> &[TestNode] {
        &self.validators
    }

    /// Returns config encoding the network structure usable for creating the genesis block of
    /// a blockchain.
    pub fn genesis_config(&self) -> GenesisConfig {
        GenesisConfig::new(self.validators.iter().map(TestNode::public_keys))
    }

    /// Updates the test network by the new set of nodes.
    pub fn update<I: IntoIterator<Item = TestNode>>(&mut self, mut us: TestNode, validators: I) {
        let validators = validators
            .into_iter()
            .enumerate()
            .map(|(id, mut validator)| {
                let validator_id = ValidatorId(id as u16);
                validator.change_role(Some(validator_id));
                if us.public_keys().consensus_key == validator.public_keys().consensus_key {
                    us.change_role(Some(validator_id));
                }
                validator
            })
            .collect::<Vec<_>>();
        self.validators = validators;
        self.us.clone_from(&us);
    }

    /// Updates the test network with a new configuration.
    pub fn update_configuration(&mut self, config: TestNetworkConfiguration) {
        self.update(config.us, config.validators);
    }

    /// Returns service public key of the validator with given id.
    pub fn service_public_key_of(&self, id: ValidatorId) -> Option<&crypto::PublicKey> {
        self.validators()
            .get(id.0 as usize)
            .map(|x| &x.service_public_key)
    }

    /// Returns consensus public key of the validator with given id.
    pub fn consensus_public_key_of(&self, id: ValidatorId) -> Option<&crypto::PublicKey> {
        self.validators()
            .get(id.0 as usize)
            .map(|x| &x.consensus_public_key)
    }
}

/// An emulated node in the test network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestNode {
    consensus_secret_key: crypto::SecretKey,
    consensus_public_key: crypto::PublicKey,
    service_secret_key: crypto::SecretKey,
    service_public_key: crypto::PublicKey,
    validator_id: Option<ValidatorId>,
}

impl TestNode {
    /// Creates a new auditor.
    pub fn new_auditor() -> Self {
        let (consensus_public_key, consensus_secret_key) = crypto::gen_keypair();
        let (service_public_key, service_secret_key) = crypto::gen_keypair();

        TestNode {
            consensus_secret_key,
            consensus_public_key,
            service_secret_key,
            service_public_key,
            validator_id: None,
        }
    }

    /// Creates a new validator with the given id.
    pub fn new_validator(validator_id: ValidatorId) -> Self {
        let (consensus_public_key, consensus_secret_key) = crypto::gen_keypair();
        let (service_public_key, service_secret_key) = crypto::gen_keypair();

        TestNode {
            consensus_secret_key,
            consensus_public_key,
            service_secret_key,
            service_public_key,
            validator_id: Some(validator_id),
        }
    }

    /// Constructs a new node from the given keypairs.
    pub fn from_parts(
        consensus_keypair: (crypto::PublicKey, crypto::SecretKey),
        service_keypair: (crypto::PublicKey, crypto::SecretKey),
        validator_id: Option<ValidatorId>,
    ) -> TestNode {
        TestNode {
            consensus_public_key: consensus_keypair.0,
            consensus_secret_key: consensus_keypair.1,
            service_public_key: service_keypair.0,
            service_secret_key: service_keypair.1,
            validator_id,
        }
    }

    /// Creates a `Propose` message signed by this validator.
    pub fn create_propose(
        &self,
        height: Height,
        last_hash: &crypto::Hash,
        tx_hashes: &[crypto::Hash],
    ) -> Propose {
        Propose::new(
            self.validator_id
                .expect("An attempt to create propose from a non-validator node."),
            height,
            Round::first(),
            last_hash,
            tx_hashes,
            &self.consensus_secret_key,
        )
    }

    /// Creates a `Precommit` message signed by this validator.
    pub fn create_precommit(&self, propose: &Propose, block_hash: &crypto::Hash) -> Precommit {
        use std::time::SystemTime;

        Precommit::new(
            self.validator_id
                .expect("An attempt to create propose from a non-validator node."),
            propose.height(),
            propose.round(),
            &propose.hash(),
            block_hash,
            SystemTime::now().into(),
            &self.consensus_secret_key,
        )
    }

    /// Returns public keys of the node.
    pub fn public_keys(&self) -> ValidatorKeys {
        ValidatorKeys {
            consensus_key: self.consensus_public_key,
            service_key: self.service_public_key,
        }
    }

    /// Returns the current validator id of node if it is validator of the test network.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_id
    }

    /// Changes node role.
    pub fn change_role(&mut self, role: Option<ValidatorId>) {
        self.validator_id = role;
    }

    /// Returns the service keypair.
    pub fn service_keypair(&self) -> (&crypto::PublicKey, &crypto::SecretKey) {
        (&self.service_public_key, &self.service_secret_key)
    }
}

impl From<TestNode> for ValidatorKeys {
    fn from(node: TestNode) -> Self {
        node.public_keys()
    }
}

/// A configuration of the test network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestNetworkConfiguration {
    us: TestNode,
    validators: Vec<TestNode>,
    stored_configuration: StoredConfiguration,
}

impl TestNetworkConfiguration {
    pub(crate) fn new(
        network: &TestNetwork,
        mut stored_configuration: StoredConfiguration,
    ) -> Self {
        let prev_hash = CryptoHash::hash(&stored_configuration);
        stored_configuration.previous_cfg_hash = prev_hash;

        TestNetworkConfiguration {
            us: network.us().clone(),
            validators: network.validators().into(),
            stored_configuration,
        }
    }

    /// Returns the node from whose perspective the testkit operates.
    pub fn us(&self) -> &TestNode {
        &self.us
    }

    /// Modifies the node from whose perspective the testkit operates.
    pub fn set_us(&mut self, us: TestNode) {
        self.us = us;
        self.update_our_role();
    }

    /// Returns the test network validators.
    pub fn validators(&self) -> &[TestNode] {
        self.validators.as_ref()
    }

    /// Returns the current consensus configuration.
    pub fn consensus_configuration(&self) -> &ConsensusConfig {
        &self.stored_configuration.consensus
    }

    /// Return the height, starting from which this configuration becomes actual.
    pub fn actual_from(&self) -> Height {
        self.stored_configuration.actual_from
    }

    /// Modifies the height, starting from which this configuration becomes actual.
    pub fn set_actual_from(&mut self, actual_from: Height) {
        self.stored_configuration.actual_from = actual_from;
    }

    /// Modifies number of votes required to accept a new consensus configuration
    /// (see majority_count field of the StoredConfiguration documentation).
    pub fn set_majority_count(&mut self, majority_count: Option<u16>) {
        self.stored_configuration.majority_count = majority_count;
    }

    /// Modifies the current consensus configuration.
    pub fn set_consensus_configuration(&mut self, consensus: ConsensusConfig) {
        self.stored_configuration.consensus = consensus;
    }

    /// Modifies the validators list.
    pub fn set_validators<I>(&mut self, validators: I)
    where
        I: IntoIterator<Item = TestNode>,
    {
        self.validators = validators
            .into_iter()
            .enumerate()
            .map(|(idx, mut node)| {
                node.change_role(Some(ValidatorId(idx as u16)));
                node
            })
            .collect();
        self.stored_configuration.validator_keys = self.validators
            .iter()
            .cloned()
            .map(ValidatorKeys::from)
            .collect();
        self.update_our_role();
    }

    /// Returns the configuration for service with the given identifier.
    pub fn service_config<D>(&self, id: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        let value = self.stored_configuration
            .services
            .get(id)
            .expect("Unable to find configuration for service");
        serde_json::from_value(value.clone()).unwrap()
    }

    /// Modifies the configuration of the service with the given identifier.
    pub fn set_service_config<D>(&mut self, id: &str, config: D)
    where
        D: Serialize,
    {
        let value = serde_json::to_value(config).unwrap();
        self.stored_configuration.services.insert(id.into(), value);
    }

    /// Returns the resulting exonum blockchain configuration.
    pub fn stored_configuration(&self) -> &StoredConfiguration {
        &self.stored_configuration
    }

    fn update_our_role(&mut self) {
        let validator_id = self.validators
            .iter()
            .position(|x| x.public_keys().service_key == self.us.service_public_key)
            .map(|x| ValidatorId(x as u16));
        self.us.validator_id = validator_id;
    }
}
