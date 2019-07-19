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

use exonum_merkledb::BinaryValue;
use serde_derive::{Deserialize, Serialize};

use std::{borrow::Cow, fmt::Display, str::FromStr};

use crate::proto::schema;

/// Service id type.
pub type ServiceInstanceId = u32;
/// Method id type.
pub type MethodId = u32;

/// Unique service transaction identifier.
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "schema::runtime::CallInfo", crate = "crate")]
pub struct CallInfo {
    /// Service instance identifier.
    pub instance_id: ServiceInstanceId,
    /// Identifier of method in service interface to call.
    pub method_id: MethodId,
}

impl CallInfo {
    /// Creates a new `CallInfo` instance.
    pub fn new(instance_id: u32, method_id: u32) -> Self {
        Self {
            instance_id,
            method_id,
        }
    }
}

/// Transaction with call info.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert)]
#[exonum(pb = "schema::runtime::AnyTx", crate = "crate")]
pub struct AnyTx {
    /// Dispatch info.
    pub call_info: CallInfo,
    /// Serialized transaction.
    pub payload: Vec<u8>,
}

impl AnyTx {
    /// Parses transaction content as concrete type.
    pub fn parse<T: BinaryValue>(&self) -> Result<T, failure::Error> {
        T::from_bytes(Cow::Borrowed(&self.payload))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert, Serialize, Deserialize)]
#[exonum(pb = "schema::runtime::InstanceSpec", crate = "crate")]
pub struct InstanceSpec {
    pub id: ServiceInstanceId,
    pub artifact: ArtifactId,
    pub name: String,
}

#[derive(
    Debug, Clone, ProtobufConvert, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[exonum(pb = "schema::runtime::ArtifactId", crate = "crate")]
pub struct ArtifactId {
    pub runtime_id: u32,
    pub name: String,
}

impl ArtifactId {
    /// Creates a new artifact identifier from the given runtime id and name.
    pub fn new(runtime_id: impl Into<u32>, name: impl Into<String>) -> Self {
        Self {
            runtime_id: runtime_id.into(),
            name: name.into(),
        }
    }
}

impl_binary_key_for_binary_value! { ArtifactId }

impl From<(String, u32)> for ArtifactId {
    fn from(v: (String, u32)) -> Self {
        Self {
            runtime_id: v.1,
            name: v.0,
        }
    }
}

impl Display for ArtifactId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.runtime_id, self.name)
    }
}

impl FromStr for ArtifactId {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.split(':').take(2).collect::<Vec<_>>();
        match &split[..] {
            [runtime_id, name] => Ok(Self {
                runtime_id: runtime_id.parse()?,
                name: name.to_string(),
            }),
            _ => Err(failure::format_err!(
                "Wrong artifact id format, it should be in form \"runtime_id:artifact_name\""
            )),
        }
    }
}

#[test]
fn parse_artifact_id_correct() {
    "0:my-service/1.0.0".parse::<ArtifactId>().unwrap();
}
