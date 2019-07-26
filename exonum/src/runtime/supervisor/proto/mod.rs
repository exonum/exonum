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

use crate::{helpers::Height, impl_serde_hex_for_binary_value, proto::Any, runtime::ArtifactId};

pub mod schema;

// Request for the artifact deployment.
#[derive(Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "schema::DeployRequest", crate = "crate")]
pub struct DeployRequest {
    // Artifact identifier.
    pub artifact: ArtifactId,
    /// Additional information for Runtime to deploy.
    pub spec: Any,
    /// The height until which the deployment procedure should be completed.
    pub deadline_height: Height,
}

// Request for the artifact deployment.
#[derive(Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "schema::DeployConfirmation", crate = "crate")]
pub struct DeployConfirmation {
    // Artifact identifier.
    pub artifact: ArtifactId,
    /// Additional information for Runtime to deploy.
    pub spec: Any,
    /// The height until which the deployment procedure should be completed.
    pub deadline_height: Height,
}

// Request for the artifact deployment.
#[derive(Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "schema::StartService", crate = "crate")]
pub struct StartService {
    /// Artifact identifier.
    pub artifact: ArtifactId,
    /// Instance name.
    pub name: String,
    /// Instance configuration.
    pub config: Any,
    /// The height until which the start service procedure should be completed.
    pub deadline_height: Height,
}

impl_binary_key_for_binary_value! { DeployRequest }
impl_binary_key_for_binary_value! { DeployConfirmation }
impl_binary_key_for_binary_value! { StartService }

impl_serde_hex_for_binary_value! { DeployRequest }
impl_serde_hex_for_binary_value! { DeployConfirmation }
impl_serde_hex_for_binary_value! { StartService }

impl From<DeployRequest> for DeployConfirmation {
    fn from(v: DeployRequest) -> Self {
        Self {
            artifact: v.artifact,
            deadline_height: v.deadline_height,
            spec: v.spec,
        }
    }
}
