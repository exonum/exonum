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

use protobuf::well_known_types::Any;

use crate::{helpers::Height, proto::schema, runtime::ArtifactId};

// Request for the artifact deployment.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "schema::supervisor::DeployArtifact", crate = "crate")]
pub struct DeployArtifact {
    // Artifact identifier.
    pub artifact: ArtifactId,
    // The height to which the deployment procedure should be completed.
    pub deadline_height: Height,
}

// Request for the artifact deployment.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "schema::supervisor::StartService", crate = "crate")]
pub struct StartService {
    /// Artifact identifier.
    pub artifact: ArtifactId,
    /// Instance name.
    pub name: String,
    /// Instance configuration.
    // TODO Write convienent wrapper [ECR-3222]
    pub config: Any,
}

// Think about bincode instead of protobuf. [ECR-3222]
macro_rules! impl_binary_key_for_binary_value {
    ($type:ident) => {
        impl exonum_merkledb::BinaryKey for $type {
            fn size(&self) -> usize {
                exonum_merkledb::BinaryValue::to_bytes(self).len()
            }

            fn write(&self, buffer: &mut [u8]) -> usize {
                let bytes = exonum_merkledb::BinaryValue::to_bytes(self);
                buffer.copy_from_slice(&bytes);
                bytes.len()
            }

            fn read(buffer: &[u8]) -> Self::Owned {
                // `unwrap` is safe because only this code uses for
                // serialize and deserialize these keys.
                <Self as exonum_merkledb::BinaryValue>::from_bytes(buffer.into()).unwrap()
            }
        }
    };
}

impl_binary_key_for_binary_value! { DeployArtifact }
impl_binary_key_for_binary_value! { StartService }
