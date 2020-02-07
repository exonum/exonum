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

use serde::{Deserialize, Serialize};

use exonum::{
    blockchain::{ConsensusConfig, ValidatorKeys},
    crypto::{self, Hash, KeyPair, PublicKey},
    helpers::{Height, Round, ValidatorId},
    keys::Keys,
    messages::{Precommit, Verified},
};

// TODO Refactor TestNetwork and TestkitBuilder [ECR-3222]

/// Emulated test network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestNetwork {
    us: TestNode,
    nodes: Vec<TestNode>,
}

impl TestNetwork {
    /// Creates a new emulated network.
    pub fn new(validator_count: u16) -> Self {
        Self::with_our_role(Some(ValidatorId(0)), validator_count)
    }

    /// Creates a new emulated network with a specific role of the node
    /// the network will be viewed from.
    pub fn with_our_role(us: Option<ValidatorId>, validator_count: u16) -> Self {
        let keys = (0..validator_count).map(|_| Keys::random());
        Self::with_our_role_from_keys(us, keys)
    }

    pub(crate) fn with_our_role_from_keys(
        us: Option<ValidatorId>,
        keys: impl IntoIterator<Item = Keys>,
    ) -> Self {
        let mut nodes = keys
            .into_iter()
            .enumerate()
            .map(|(n, keys)| TestNode {
                keys,
                validator_id: Some(ValidatorId(n as u16)),
            })
            .collect::<Vec<_>>();

        assert!(
            !nodes.is_empty(),
            "At least one validator should be present in the network."
        );

        let us = if let Some(ValidatorId(id)) = us {
            nodes[id as usize].clone()
        } else {
            let us = TestNode::new_auditor();
            nodes.push(us.clone());
            us
        };

        Self { nodes, us }
    }

    /// Adds a new auditor node to this network.
    pub fn add_node(&mut self) -> &TestNode {
        self.nodes.push(TestNode::new_auditor());
        self.nodes.last().unwrap()
    }

    /// Returns the node in the emulated network, from whose perspective the testkit operates.
    pub fn us(&self) -> &TestNode {
        &self.us
    }

    /// Returns all validators in the network.
    pub fn validators(&self) -> Vec<TestNode> {
        let mut validators = self
            .nodes
            .iter()
            .filter(|x| x.validator_id.is_some())
            .cloned()
            .collect::<Vec<_>>();
        validators.sort_by(|a, b| a.validator_id.cmp(&b.validator_id));
        validators
    }

    /// Returns a slice of all nodes in the network.
    pub fn nodes(&self) -> &[TestNode] {
        &self.nodes
    }

    /// Returns config encoding the network structure usable for creating the genesis block of
    /// a blockchain.
    pub fn genesis_config(&self) -> ConsensusConfig {
        let validator_keys = self
            .validators()
            .iter()
            .map(TestNode::public_keys)
            .collect();
        ConsensusConfig::default().with_validator_keys(validator_keys)
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
        self.nodes = validators;
        self.us.clone_from(&us);
    }

    /// Updates the test network with a new consensus configuration.
    // TODO Optimize O(n^2) [ECR-3222]
    pub fn update_consensus_config(&mut self, config: ConsensusConfig) {
        // Assign new node roles.
        for node in &mut self.nodes {
            node.validator_id = config
                .find_validator(|keys| keys.consensus_key == node.consensus_keypair().public_key());
        }
        // Verify that all validator keys have been assigned.
        let validators_count = self
            .nodes
            .iter()
            .filter(|x| x.validator_id.is_some())
            .count();
        assert_eq!(validators_count, config.validator_keys.len());
        // Modify us.
        self.us.validator_id = config
            .find_validator(|keys| keys.consensus_key == self.us.consensus_keypair().public_key());
    }

    /// Returns service public key of the validator with given id.
    pub fn service_public_key_of(&self, id: ValidatorId) -> Option<PublicKey> {
        self.validators()
            .get(id.0 as usize)
            .map(|x| x.keys.service_pk())
    }

    /// Returns consensus public key of the validator with given id.
    pub fn consensus_public_key_of(&self, id: ValidatorId) -> Option<PublicKey> {
        self.validators()
            .get(id.0 as usize)
            .map(|x| x.keys.consensus_pk())
    }
}

/// An emulated node in the test network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestNode {
    keys: Keys,
    validator_id: Option<ValidatorId>,
}

impl TestNode {
    /// Creates a new auditor.
    pub fn new_auditor() -> Self {
        TestNode {
            keys: Keys::random(),
            validator_id: None,
        }
    }

    /// Creates a new validator with the given id.
    pub fn new_validator(validator_id: ValidatorId) -> Self {
        TestNode {
            keys: Keys::random(),
            validator_id: Some(validator_id),
        }
    }

    /// Constructs a new node from the given keypairs.
    pub fn from_parts(
        consensus_keys: impl Into<KeyPair>,
        service_keys: impl Into<KeyPair>,
        validator_id: Option<ValidatorId>,
    ) -> TestNode {
        TestNode {
            keys: Keys::from_keys(consensus_keys, service_keys),
            validator_id,
        }
    }

    /// Creates a `Precommit` message signed by this validator.
    pub fn create_precommit(
        &self,
        height: Height,
        block_hash: crypto::Hash,
    ) -> Verified<Precommit> {
        use std::time::SystemTime;

        Verified::from_value(
            Precommit::new(
                self.validator_id
                    .expect("An attempt to create propose from a non-validator node."),
                height,
                Round::first(),
                Hash::zero(),
                block_hash,
                SystemTime::now().into(),
            ),
            self.keys.consensus_pk(),
            &self.keys.consensus_sk(),
        )
    }

    /// Returns public keys of the node.
    pub fn public_keys(&self) -> ValidatorKeys {
        ValidatorKeys::new(self.keys.consensus_pk(), self.keys.service_pk())
    }

    /// Returns the current validator id of node if it is validator of the test network.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_id
    }

    /// Changes node role.
    pub fn change_role(&mut self, role: Option<ValidatorId>) {
        self.validator_id = role;
    }

    /// Returns the service keypair of the node.
    pub fn service_keypair(&self) -> KeyPair {
        self.keys.service.clone()
    }

    /// Returns the consensus keypair of the node.
    pub fn consensus_keypair(&self) -> KeyPair {
        self.keys.consensus.clone()
    }
}

impl From<TestNode> for ValidatorKeys {
    fn from(node: TestNode) -> Self {
        node.public_keys()
    }
}
