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

use anyhow as failure; // FIXME: remove once `ProtobufConvert` derive is improved (ECR-4316)
use exonum::{
    blockchain::ConsensusConfig,
    crypto::Hash,
    helpers::Height,
    merkledb::{impl_binary_key_for_binary_value, BinaryValue, ObjectHash},
    runtime::{ArtifactId, ExecutionStatus, InstanceId, InstanceSpec, MigrationStatus},
};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_proto::{ProtobufBase64, ProtobufConvert};
use serde_derive::{Deserialize, Serialize};

use super::{mode::Mode, proto};

/// Supervisor service configuration (not to be confused with `ConfigPropose`, which
/// contains core/service configuration change proposal).
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Config")]
#[non_exhaustive]
pub struct SupervisorConfig {
    /// Supervisor operating mode.
    pub mode: Mode,
}

impl SupervisorConfig {
    /// Creates a new configuration with the specified supervisor mode.
    pub fn new(mode: Mode) -> Self {
        Self { mode }
    }
}

/// Request for the artifact deployment.
#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[derive(Serialize, Deserialize)]
#[protobuf_convert(source = "proto::DeployRequest")]
#[non_exhaustive]
pub struct DeployRequest {
    /// Artifact identifier.
    pub artifact: ArtifactId,

    /// Additional information for the runtime necessary to deploy the artifact.
    #[serde(with = "ProtobufBase64")]
    pub spec: Vec<u8>,

    /// The height until which the deployment procedure should be completed.
    pub deadline_height: Height,

    /// Seed to allow several deployments with the same params.
    #[serde(default)]
    pub seed: u64,
}

impl DeployRequest {
    /// Creates a deploy request with an empty artifact specification.
    pub fn new(artifact: ArtifactId, deadline_height: Height) -> Self {
        Self {
            artifact,
            deadline_height,
            spec: Vec::new(),
            seed: 0,
        }
    }

    /// Sets the artifact specification for this request.
    pub fn with_spec(mut self, spec: Vec<u8>) -> Self {
        self.spec = spec;
        self
    }
}

/// Confirmation that artifact deployment has ended for a validator.
/// Result can be either successful or unsuccessful.
#[derive(Debug, Clone, BinaryValue, ObjectHash, ProtobufConvert)]
#[protobuf_convert(source = "proto::DeployResult")]
#[non_exhaustive]
pub struct DeployResult {
    /// Corresponding request.
    pub request: DeployRequest,
    /// Result of deployment.
    pub result: ExecutionStatus,
}

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

/// Request to start a new service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash, Serialize, Deserialize)]
#[protobuf_convert(source = "proto::StartService")]
#[non_exhaustive]
pub struct StartService {
    /// Artifact identifier.
    pub artifact: ArtifactId,

    /// Instance name.
    pub name: String,

    /// Instance configuration.
    #[serde(with = "ProtobufBase64")]
    pub config: Vec<u8>,
}

impl StartService {
    /// Given the instance ID, splits the `StartService` request into `InstanceSpec`
    /// and config value.
    pub fn into_parts(self, id: InstanceId) -> (InstanceSpec, Vec<u8>) {
        let spec = InstanceSpec::from_raw_parts(id, self.name, self.artifact);

        (spec, self.config)
    }
}

/// Request to stop an existing service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash, Serialize, Deserialize)]
#[protobuf_convert(source = "proto::StopService")]
#[non_exhaustive]
pub struct StopService {
    /// Corresponding service instance ID.
    pub instance_id: InstanceId,
}

/// Request to freeze an existing service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash, Serialize, Deserialize)]
#[protobuf_convert(source = "proto::FreezeService")]
#[non_exhaustive]
pub struct FreezeService {
    /// Corresponding service instance ID.
    pub instance_id: InstanceId,
}

/// Request to resume a previously stopped service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash, Serialize, Deserialize)]
#[protobuf_convert(source = "proto::ResumeService")]
#[non_exhaustive]
pub struct ResumeService {
    /// Corresponding service instance ID.
    pub instance_id: InstanceId,

    /// Raw bytes representation of service resume parameters.
    #[serde(with = "ProtobufBase64")]
    pub params: Vec<u8>,
}

/// Request to unload an unused artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash, Serialize, Deserialize)]
#[protobuf_convert(source = "proto::UnloadArtifact")]
pub struct UnloadArtifact {
    /// Artifact identifier.
    pub artifact_id: ArtifactId,
}

/// Configuration parameters of the certain service instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash, Serialize, Deserialize)]
#[protobuf_convert(source = "proto::ServiceConfig")]
#[non_exhaustive]
pub struct ServiceConfig {
    /// Corresponding service instance ID.
    pub instance_id: InstanceId,

    /// Raw bytes representation of the service configuration parameters.
    #[serde(with = "ProtobufBase64")]
    pub params: Vec<u8>,
}

impl ServiceConfig {
    /// Creates a new configuration request.
    pub fn new(instance_id: InstanceId, params: impl BinaryValue) -> Self {
        Self {
            instance_id,
            params: params.into_bytes(),
        }
    }
}

/// Atomic configuration change.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::ConfigChange", rename(case = "snake_case"))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
    /// Request to freeze an existing service instance.
    FreezeService(FreezeService),
    /// Request to unload an unused artifact.
    UnloadArtifact(UnloadArtifact),
}

/// Request for the configuration change
#[derive(Debug, Clone, Eq, PartialEq)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash, Serialize, Deserialize)]
#[protobuf_convert(source = "proto::ConfigPropose")]
#[non_exhaustive]
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

    /// Adds a service start request to this proposal.
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

    /// Adds a service stop request to this proposal.
    pub fn stop_service(mut self, instance_id: InstanceId) -> Self {
        self.changes
            .push(ConfigChange::StopService(StopService { instance_id }));
        self
    }

    /// Adds a service freeze request to this proposal.
    pub fn freeze_service(mut self, instance_id: InstanceId) -> Self {
        self.changes
            .push(ConfigChange::FreezeService(FreezeService { instance_id }));
        self
    }

    /// Adds a service resume request to this proposal.
    pub fn resume_service(mut self, instance_id: InstanceId, params: impl BinaryValue) -> Self {
        self.changes
            .push(ConfigChange::ResumeService(ResumeService {
                instance_id,
                params: params.into_bytes(),
            }));
        self
    }

    /// Adds an artifact unloading request to this proposal.
    pub fn unload_artifact(mut self, artifact_id: ArtifactId) -> Self {
        self.changes
            .push(ConfigChange::UnloadArtifact(UnloadArtifact { artifact_id }));
        self
    }
}

/// Confirmation vote for the configuration change.
#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[derive(Serialize, Deserialize)]
#[protobuf_convert(source = "proto::ConfigVote")]
#[non_exhaustive]
pub struct ConfigVote {
    /// Hash of configuration proposition.
    pub propose_hash: Hash,
}

impl ConfigVote {
    /// Creates a vote for the proposal with the specified hash.
    pub fn new(propose_hash: Hash) -> Self {
        Self { propose_hash }
    }
}

impl From<ConfigPropose> for ConfigVote {
    fn from(propose: ConfigPropose) -> Self {
        Self {
            propose_hash: propose.object_hash(),
        }
    }
}

/// Request for the service data migration.
#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[derive(Serialize, Deserialize)]
#[protobuf_convert(source = "proto::MigrationRequest")]
#[non_exhaustive]
pub struct MigrationRequest {
    /// New artifact identifier.
    pub new_artifact: ArtifactId,

    /// Name of service for a migration.
    pub service: String,

    /// The height until which the migration procedure should be completed.
    pub deadline_height: Height,

    /// Seed to allow several migrations with the same params.
    #[serde(default)]
    pub seed: u64,
}

impl MigrationRequest {
    /// Creates a new migration request.
    pub fn new(
        new_artifact: ArtifactId,
        service: impl Into<String>,
        deadline_height: Height,
    ) -> Self {
        Self {
            new_artifact,
            service: service.into(),
            deadline_height,
            seed: 0,
        }
    }
}

/// Confirmation that migration has ended for a validator.
/// Result can be either successful or unsuccessful.
#[derive(Debug, Clone, BinaryValue, ObjectHash, ProtobufConvert)]
#[protobuf_convert(source = "proto::MigrationResult")]
#[non_exhaustive]
pub struct MigrationResult {
    /// Corresponding request.
    pub request: MigrationRequest,
    /// Result of migration.
    pub status: MigrationStatus,
}

impl MigrationResult {
    /// Creates a migration result.
    pub fn new(request: MigrationRequest, result: impl Into<MigrationStatus>) -> Self {
        Self {
            request,
            status: result.into(),
        }
    }
}

/// Pending config change proposal entry
#[derive(Clone, Debug, Eq, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::ConfigProposalWithHash")]
#[non_exhaustive]
pub struct ConfigProposalWithHash {
    /// Hash of configuration proposition.
    pub propose_hash: Hash,
    /// The configuration change proposal
    pub config_propose: ConfigPropose,
}

impl_binary_key_for_binary_value! { DeployRequest }
impl_binary_key_for_binary_value! { MigrationRequest }
