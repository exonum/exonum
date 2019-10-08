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
    crypto::Hash,
    exonum_merkledb::ObjectHash,
    helpers::Height,
    impl_serde_hex_for_binary_value,
    runtime::{ArtifactId, ConfigChange},
};

use super::proto;

/// Request for the artifact deployment.
#[derive(Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::DeployRequest")]
pub struct DeployRequest {
    /// Artifact identifier.
    pub artifact: ArtifactId,
    /// Additional information for Runtime to deploy.
    pub spec: Vec<u8>,
    /// The height until which the deployment procedure should be completed.
    pub deadline_height: Height,
}

/// Request for the artifact deployment.
#[derive(Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::DeployConfirmation")]
pub struct DeployConfirmation {
    /// Artifact identifier.
    pub artifact: ArtifactId,
    /// Additional information for Runtime to deploy.
    pub spec: Vec<u8>,
    /// The height until which the deployment procedure should be completed.
    pub deadline_height: Height,
}

/// Request for the artifact deployment.
#[derive(Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::StartService")]
pub struct StartService {
    /// Artifact identifier.
    pub artifact: ArtifactId,
    /// Instance name.
    pub name: String,
    /// Instance configuration.
    pub config: Vec<u8>,
    /// The height until which the start service procedure should be completed.
    pub deadline_height: Height,
}

/// Request for the configuration change
#[derive(Debug, Clone, Eq, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::ConfigPropose")]
pub struct ConfigPropose {
    /// The height until which the update configuration procedure should be completed.
    pub actual_from: Height,
    /// New configuration proposition.
    pub changes: Vec<ConfigChange>,
}

/// Confirmation vote for the configuration change
#[derive(Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::ConfigVote")]
pub struct ConfigVote {
    /// Hash of configuration proposition.
    pub propose_hash: Hash,
}

/// Pending config change proposal entry
#[derive(Clone, Debug, Eq, PartialEq, ProtobufConvert, Serialize, Deserialize)]
#[exonum(pb = "proto::ConfigProposalWithHash")]
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
            deadline_height: v.deadline_height,
            spec: v.spec,
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
