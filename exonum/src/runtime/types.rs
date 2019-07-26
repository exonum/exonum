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

use exonum_merkledb::{is_allowed_latin1_char, is_valid_index_name, BinaryValue};
use serde_derive::{Deserialize, Serialize};

use std::{borrow::Cow, fmt::Display, str::FromStr};

use crate::{helpers::ValidateInput, proto::schema};

/// Service id type.
pub type ServiceInstanceId = u32;
/// Method id type.
pub type MethodId = u32;

/// Unique service transaction identifier.
#[derive(
    Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert, Serialize, Deserialize,
)]
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

/// Transaction with information to call.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug, ProtobufConvert, Serialize, Deserialize)]
#[exonum(pb = "schema::runtime::AnyTx", crate = "crate")]
pub struct AnyTx {
    /// Information to call.
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

#[derive(
    Debug, Clone, ProtobufConvert, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[exonum(pb = "schema::runtime::ArtifactId", crate = "crate")]
pub struct ArtifactId {
    pub runtime_id: u32,
    pub name: String,
}

impl ArtifactId {
    /// Creates a new artifact identifier from the given runtime id and name
    /// or returns error if the resulting artifact id is not correct.
    pub fn new(
        runtime_id: impl Into<u32>,
        name: impl Into<String>,
    ) -> Result<Self, failure::Error> {
        let artifact = Self {
            runtime_id: runtime_id.into(),
            name: name.into(),
        };
        artifact.validate()?;
        Ok(artifact)
    }

    /// Checks that the artifact name contains only allowed characters and is not empty.
    fn is_valid_name(name: impl AsRef<[u8]>) -> bool {
        // Extended version of `exonum_merkledb::is_valid_name` that allows also '/`.
        name.as_ref().iter().all(|&c| match c {
            47 => true,
            c => is_allowed_latin1_char(c),
        })
    }
}

impl ValidateInput for ArtifactId {
    type Error = failure::Error;

    fn validate(&self) -> Result<(), Self::Error> {
        ensure!(!self.name.is_empty(), "Artifact name should not be empty");
        ensure!(
            Self::is_valid_name(&self.name),
            "Artifact name contains illegal character, use only: a-zA-Z0-9 and one of _-./"
        );
        Ok(())
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
        let split = s.split(':').collect::<Vec<_>>();
        match &split[..] {
            [runtime_id, name] => {
                let artifact = Self {
                    runtime_id: runtime_id.parse()?,
                    name: name.to_string(),
                };
                artifact.validate()?;
                Ok(artifact)
            }
            _ => Err(failure::format_err!(
                "Wrong artifact id format, it should be in form \"runtime_id:artifact_name\""
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert, Serialize, Deserialize)]
#[exonum(pb = "schema::runtime::InstanceSpec", crate = "crate")]
pub struct InstanceSpec {
    pub id: ServiceInstanceId,
    pub artifact: ArtifactId,
    pub name: String,
}

impl InstanceSpec {
    /// Creates a new instance specification or returns an error
    /// if the resulting specification is not correct.
    pub fn new(
        id: ServiceInstanceId,
        name: impl Into<String>,
        artifact: impl AsRef<str>,
    ) -> Result<Self, failure::Error> {
        let spec = Self {
            id,
            artifact: artifact.as_ref().parse()?,
            name: name.into(),
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Checks that the instance name contains only allowed characters and is not empty.
    pub fn is_valid_name(name: impl AsRef<str>) -> Result<(), failure::Error> {
        let name = name.as_ref();
        ensure!(
            !name.is_empty(),
            "Service instance name should not be empty"
        );
        ensure!(
            is_valid_index_name(name),
            "Service instance name contains illegal character, use only: a-zA-Z0-9 and one of _-."
        );
        Ok(())
    }
}

impl ValidateInput for InstanceSpec {
    type Error = failure::Error;

    fn validate(&self) -> Result<(), Self::Error> {
        self.artifact.validate()?;
        Self::is_valid_name(&self.name)
    }
}

#[test]
fn parse_artifact_id_correct() {
    "0:my-service/1.0.0".parse::<ArtifactId>().unwrap();
    "1:com.my.java.service.v1".parse::<ArtifactId>().unwrap();
}

#[test]
fn parse_artifact_id_incorrect_layout() {
    let artifacts = [
        ("0:3:my-service/1.0.0", "Wrong artifact id format"),
        ("my-service/1.0.0", "Wrong artifact id format"),
        ("15", "Wrong artifact id format"),
        ("0:", "Artifact name should not be empty"),
        (":", "cannot parse integer from empty string"),
        (":123", "cannot parse integer from empty string"),
        ("-1:123", "invalid digit found in string"),
        ("ava:123", "invalid digit found in string"),
        (
            "123:I am a service!",
            "Artifact name contains illegal character",
        ),
        // cspell:ignore юникоды
        (
            "123:юникоды!",
            "Artifact name contains illegal character",
        ),
    ];

    for (artifact, expected_err) in &artifacts {
        let actual_err = artifact.parse::<ArtifactId>().unwrap_err().to_string();
        assert!(
            actual_err.contains(expected_err),
            "artifact: '{}' actual_err '{}', expected_err '{}'",
            artifact,
            actual_err,
            expected_err
        );
    }
}

#[test]
fn test_instance_spec_validate_correct() {
    InstanceSpec::new(15, "foo-service", "0:my-service/1.0.0").unwrap();
}

#[test]
fn test_instance_spec_validate_incorrect() {
    let specs = [
        (
            InstanceSpec::new(1, "", "0:my-service/1.0.0"),
            "Service instance name should not be empty",
        ),
        // cspell:ignore русский сервис
        (
            InstanceSpec::new(2, "русский_сервис", "0:my-service/1.0.0"),
            "Service instance name contains illegal character",
        ),
        (
            InstanceSpec::new(3, "space service", "1:java.runtime.service"),
            "Service instance name contains illegal character",
        ),
        (
            InstanceSpec::new(4, "foo_service", ""),
            "Wrong artifact id format",
        ),
        (
            InstanceSpec::new(5, "foo_service", ":"),
            "cannot parse integer from empty string",
        ),
    ];

    for (instance_spec, expected_err) in &specs {
        let actual_err = instance_spec.as_ref().unwrap_err().to_string();
        assert!(
            actual_err.contains(expected_err),
            "actual_err '{:?}', expected_err '{}'",
            instance_spec,
            expected_err,
        );
    }
}
