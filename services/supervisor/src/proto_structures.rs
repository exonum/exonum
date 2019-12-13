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

use serde_derive::{Deserialize, Serialize};

use exonum::{
    blockchain::ConsensusConfig,
    crypto::Hash,
    exonum_merkledb::ObjectHash,
    helpers::Height,
    impl_serde_hex_for_binary_value,
    messages::{AnyTx, Verified},
    runtime::{rust::Transaction, ArtifactId, InstanceId, InstanceSpec, SUPERVISOR_INSTANCE_ID},
};
use exonum_crypto::{PublicKey, SecretKey};
use exonum_derive::*;
use exonum_merkledb::{impl_binary_key_for_binary_value, BinaryValue};
use exonum_proto::ProtobufConvert;

use super::{mode::Mode, proto, transactions::SupervisorInterface};

/// Supervisor service configuration (not to be confused with `ConfigPropose`, which
/// contains core/service configuration change proposal).
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Config")]
pub struct SupervisorConfig {
    /// Supervisor operating mode.
    pub mode: Mode,
}

/// Request for the artifact deployment.
#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::DeployRequest")]
pub struct DeployRequest {
    /// Artifact identifier.
    pub artifact: ArtifactId,
    /// Additional information for Runtime to deploy.
    pub spec: Vec<u8>,
    /// The height until which the deployment procedure should be completed.
    pub deadline_height: Height,
}

/// Request for the artifact deployment.
#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::DeployConfirmation")]
pub struct DeployConfirmation {
    /// Artifact identifier.
    pub artifact: ArtifactId,
}

/// Request for the artifact deployment.
#[protobuf_convert(source = "proto::StartService")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
pub struct StartService {
    /// Artifact identifier.
    pub artifact: ArtifactId,
    /// Instance name.
    pub name: String,
    /// Instance configuration.
    pub config: Vec<u8>,
}

impl StartService {
    /// Given the instance ID, splits the `StartService` request into `InstanceSpec`
    /// and config value.
    pub fn into_parts(self, id: InstanceId) -> (InstanceSpec, Vec<u8>) {
        let spec = InstanceSpec {
            id,
            name: self.name,
            artifact: self.artifact,
        };

        (spec, self.config)
    }
}

/// Configuration parameters of the certain service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::ServiceConfig")]
pub struct ServiceConfig {
    /// Corresponding service instance ID.
    pub instance_id: InstanceId,
    /// Raw bytes representation of service configuration parameters.
    pub params: Vec<u8>,
}

/// Atomic configuration change.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::ConfigChange", rename(case = "snake_case"))]
pub enum ConfigChange {
    /// New consensus config.
    Consensus(ConsensusConfig),
    /// New service instance config.
    Service(ServiceConfig),
    /// New service instance start request.
    StartService(StartService),
}

/// Request for the configuration change
#[derive(Debug, Clone, Eq, PartialEq)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::ConfigPropose")]
pub struct ConfigPropose {
    /// The height until which the update configuration procedure should be completed.
    pub actual_from: Height,
    /// New configuration proposition.
    pub changes: Vec<ConfigChange>,
    /// Configuration proposal number to avoid conflicting proposals.
    pub configuration_number: u64,
}

impl ConfigPropose {
    /// Signs the proposal for the supervisor service.
    pub fn sign_for_supervisor(
        self,
        public_key: PublicKey,
        secret_key: &SecretKey,
    ) -> Verified<AnyTx> {
        Transaction::<dyn SupervisorInterface>::sign(
            self,
            SUPERVISOR_INSTANCE_ID,
            public_key,
            secret_key,
        )
    }

    /// Creates a new proposal which activates at the specified height.
    pub fn new(configuration_number: u64, actual_from: Height) -> Self {
        Self {
            actual_from,
            changes: Vec::default(),
            configuration_number,
        }
    }

    /// Creates a new proposal which should be activated at the next height.
    pub fn immediate(configuration_number: u64) -> Self {
        Self::new(configuration_number, Height(0))
    }

    /// Adds a change of consensus configuration to this proposal.
    pub fn consensus_config(mut self, config: ConsensusConfig) -> Self {
        self.changes.push(ConfigChange::Consensus(config));
        self
    }

    /// Adds change of the configuration for the specified service instance.
    pub fn service_config(mut self, instance_id: InstanceId, config: impl BinaryValue) -> Self {
        self.changes.push(ConfigChange::Service(ServiceConfig {
            instance_id,
            params: config.into_bytes(),
        }));
        self
    }

    /// Adds service start request to this proposal.
    pub fn start_service(mut self, start_service: StartService) -> Self {
        self.changes.push(ConfigChange::StartService(start_service));
        self
    }
}

/// Confirmation vote for the configuration change
#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::ConfigVote")]
pub struct ConfigVote {
    /// Hash of configuration proposition.
    pub propose_hash: Hash,
}

/// Pending config change proposal entry
#[derive(Clone, Debug, Eq, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::ConfigProposalWithHash")]
pub struct ConfigProposalWithHash {
    /// Hash of configuration proposition.
    pub propose_hash: Hash,
    /// The configuration change proposal
    pub config_propose: ConfigPropose,
}

impl_binary_key_for_binary_value! { DeployRequest }
impl_binary_key_for_binary_value! { DeployConfirmation }
impl_binary_key_for_binary_value! { StartService }
impl_binary_key_for_binary_value! { ConfigPropose }
impl_binary_key_for_binary_value! { ConfigVote }

impl_serde_hex_for_binary_value! { DeployRequest }
impl_serde_hex_for_binary_value! { DeployConfirmation }
impl_serde_hex_for_binary_value! { StartService }
impl_serde_hex_for_binary_value! { ConfigPropose }
impl_serde_hex_for_binary_value! { ConfigVote }

impl From<DeployRequest> for DeployConfirmation {
    fn from(v: DeployRequest) -> Self {
        Self {
            artifact: v.artifact,
        }
    }
}

impl From<ConfigPropose> for ConfigVote {
    fn from(v: ConfigPropose) -> Self {
        Self {
            propose_hash: v.object_hash(),
        }
    }
}
