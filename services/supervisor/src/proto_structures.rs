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
    helpers::Height,
    merkledb::{
        impl_binary_key_for_binary_value, impl_serde_hex_for_binary_value, BinaryValue, ObjectHash,
    },
    runtime::{ArtifactId, ExecutionStatus, InstanceId, InstanceSpec, MigrationStatus},
};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use super::{mode::Mode, proto};

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

/// Confirmation that artifact deployment has ended for a validator.
/// Result can be either successful or unsuccessful.
#[derive(Debug, Clone, BinaryValue, ObjectHash, ProtobufConvert)]
#[protobuf_convert(source = "proto::DeployResult")]
pub struct DeployResult {
    /// Corresponding request.
    pub request: DeployRequest,
    /// Result of deployment.
    pub result: ExecutionStatus,
}

/// Request for the start service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::StartService")]
pub struct StartService {
    /// Artifact identifier.
    pub artifact: ArtifactId,
    /// Instance name.
    pub name: String,
    /// Instance configuration.
    pub config: Vec<u8>,
}

/// Request for the stop existing service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::StopService")]
pub struct StopService {
    /// Corresponding service instance ID.
    pub instance_id: InstanceId,
}

/// Request for the resume previously stopped service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::ResumeService")]
pub struct ResumeService {
    /// Corresponding service instance ID.
    pub instance_id: InstanceId,
    /// Updated artifact ID.
    pub artifact: ArtifactId,
    /// Raw bytes representation of service resume parameters.
    pub params: Vec<u8>,
}

impl StartService {
    /// Given the instance ID, splits the `StartService` request into `InstanceSpec`
    /// and config value.
    pub fn into_parts(self, id: InstanceId) -> (InstanceSpec, Vec<u8>) {
        let spec = InstanceSpec::from_raw_parts(id, self.name, self.artifact);

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
    /// Request to start a new service instance.
    StartService(StartService),
    /// Request to stop an existing service instance.
    StopService(StopService),
    /// Request to resume a previously stopped service instance.
    ResumeService(ResumeService),
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
    pub fn start_service(
        mut self,
        artifact: ArtifactId,
        name: impl Into<String>,
        constructor: impl BinaryValue,
    ) -> Self {
        let start_service = StartService {
            artifact,
            name: name.into(),
            config: constructor.into_bytes(),
        };

        self.changes.push(ConfigChange::StartService(start_service));
        self
    }

    /// Adds service stop request to this proposal.
    pub fn stop_service(mut self, instance_id: InstanceId) -> Self {
        self.changes
            .push(ConfigChange::StopService(StopService { instance_id }));
        self
    }

    /// Adds service resume request to this proposal.
    pub fn resume_service(
        mut self,
        instance_id: InstanceId,
        artifact: ArtifactId,
        params: impl BinaryValue,
    ) -> Self {
        self.changes
            .push(ConfigChange::ResumeService(ResumeService {
                instance_id,
                artifact,
                params: params.into_bytes(),
            }));
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

/// Request for the service data migration.
#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::MigrationRequest")]
pub struct MigrationRequest {
    /// New artifact identifier.
    pub new_artifact: ArtifactId,
    /// Name of service for a migration.
    pub service: String,
    /// The height until which the migration procedure should be completed.
    pub deadline_height: Height,
}

/// Confirmation that migration has ended for a validator.
/// Result can be either successful or unsuccessful.
#[derive(Debug, Clone, BinaryValue, ObjectHash, ProtobufConvert)]
#[protobuf_convert(source = "proto::MigrationResult")]
pub struct MigrationResult {
    /// Corresponding request.
    pub request: MigrationRequest,
    /// Result of migration.
    pub status: MigrationStatus,
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
impl_binary_key_for_binary_value! { DeployResult }
impl_binary_key_for_binary_value! { StartService }
impl_binary_key_for_binary_value! { StopService }
impl_binary_key_for_binary_value! { ResumeService }
impl_binary_key_for_binary_value! { ConfigPropose }
impl_binary_key_for_binary_value! { ConfigVote }
impl_binary_key_for_binary_value! { MigrationRequest }

impl_serde_hex_for_binary_value! { DeployRequest }
impl_serde_hex_for_binary_value! { DeployResult }
impl_serde_hex_for_binary_value! { StartService }
impl_serde_hex_for_binary_value! { StopService }
impl_serde_hex_for_binary_value! { ResumeService }
impl_serde_hex_for_binary_value! { ConfigPropose }
impl_serde_hex_for_binary_value! { ConfigVote }
impl_serde_hex_for_binary_value! { MigrationRequest }

impl DeployResult {
    /// Creates a new `DeployRequest` object with a positive result.
    pub fn ok(request: DeployRequest) -> Self {
        Self {
            request,
            result: Ok(()).into(),
        }
    }

    /// Creates a new `DeployRequest` object.
    pub fn new<R: Into<ExecutionStatus>>(request: DeployRequest, result: R) -> Self {
        Self {
            request,
            result: result.into(),
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
