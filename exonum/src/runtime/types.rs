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

use exonum_merkledb::{
    impl_binary_key_for_binary_value,
    validation::{is_allowed_index_name_char, is_valid_index_name_component},
    BinaryValue,
};
use exonum_proto::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use std::{borrow::Cow, fmt::Display, str::FromStr};

use super::InstanceDescriptor;
use crate::{blockchain::config::InstanceInitParams, helpers::ValidateInput, proto::schema};

/// Unique service instance identifier.
///
/// * This is a secondary identifier, mainly used in transaction messages.
/// The primary one is a service instance name.
///
/// * The dispatcher assigns this identifier when the service is started.
pub type InstanceId = u32;
/// Identifier of the method in the service interface required for the call.
pub type MethodId = u32;

/// Information for calling the service method.
#[derive(Default, Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert)]
#[protobuf_convert(source = "schema::runtime::CallInfo")]
pub struct CallInfo {
    /// Unique service instance identifier. The dispatcher uses this identifier to find the
    /// corresponding runtime to execute a transaction.
    pub instance_id: InstanceId,
    /// Identifier of the method in the service interface required for the call.
    pub method_id: MethodId,
}

impl CallInfo {
    /// Create an ordinary `CallInfo` instance.
    pub fn new(instance_id: u32, method_id: u32) -> Self {
        Self {
            instance_id,
            method_id,
        }
    }
}

/// Transaction with the information required for the call.
///
/// # Examples
///
/// Create a new signed transaction.
/// ```
/// use exonum::{
///     crypto,
///     messages::Verified,
///     runtime::{AnyTx, CallInfo},
/// };
///
/// let keypair = crypto::gen_keypair();
/// let transaction = Verified::from_value(
///     AnyTx {
///         call_info: CallInfo {
///             // Service instance which we want to call.
///             instance_id: 1024,
///             // Specific method of the service interface.
///             method_id: 0,
///             ..CallInfo::default()
///         },
///         // Transaction payload.
///         arguments: "Talk is cheap. Show me the code. – Linus Torvalds".to_owned().into_bytes()
///     },
///     keypair.0,
///     &keypair.1
/// );
/// ```
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert)]
#[protobuf_convert(source = "schema::runtime::AnyTx")]
pub struct AnyTx {
    /// Information required for the call of the corresponding executor.
    pub call_info: CallInfo,
    /// Serialized transaction arguments.
    pub arguments: Vec<u8>,
}

impl AnyTx {
    /// Parse transaction arguments as a specific type.
    pub fn parse<T: BinaryValue>(&self) -> Result<T, failure::Error> {
        T::from_bytes(Cow::Borrowed(&self.arguments))
    }
}

/// The artifact identifier is required by the runtime to construct service instances.
/// In other words, an artifact identifier is similar to a class name, and a specific service
/// instance is similar to a class instance.
///
/// In string representation the artifact identifier is written as follows:
///
/// `{runtime_id}:{artifact_name}`, where `runtime_id` is a [runtime identifier],
/// and `artifact_name` is a unique name of the artifact.
///
/// Artifact name contains only the following characters: `a-zA-Z0-9` and one of `_-.:`.
///
/// [runtime identifier]: enum.RuntimeIdentifier.html
///
/// # Example
///
/// ```
/// # use exonum::runtime::ArtifactId;
/// # fn main() -> Result<(), failure::Error> {
/// // Typical Rust artifact.
/// let rust_artifact_id = "0:my-service:1.0.0".parse::<ArtifactId>()?;
/// // Typical Java artifact.
/// let java_artifact_id = "1:org.exonum.service.1".parse::<ArtifactId>()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::runtime::ArtifactId")]
pub struct ArtifactId {
    /// Runtime identifier.
    pub runtime_id: u32,
    /// Unique artifact name.
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

    /// Converts into `InstanceInitParams` with given id, name and empty constructor.
    pub fn into_default_instance(
        self,
        id: InstanceId,
        name: impl Into<String>,
    ) -> InstanceInitParams {
        InstanceInitParams::new(id, name, self, ())
    }

    /// Checks that the artifact name contains only allowed characters and is not empty.
    fn is_valid_name(name: impl AsRef<[u8]>) -> bool {
        // Extended version of `exonum_merkledb::is_valid_name` that also allows '.' and ':'.
        name.as_ref().iter().all(|&c| match c {
            b'.' | b':' => true,
            c => is_allowed_index_name_char(c),
        })
    }
}

impl ValidateInput for ArtifactId {
    type Error = failure::Error;

    /// Checks that the artifact name contains only allowed characters and is not empty.
    fn validate(&self) -> Result<(), Self::Error> {
        ensure!(!self.name.is_empty(), "Artifact name should not be empty");
        ensure!(
            Self::is_valid_name(&self.name),
            "Artifact name ({}) contains an illegal character, use only: a-zA-Z0-9 and one of _-.:",
            &self.name,
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.runtime_id, self.name)
    }
}

impl FromStr for ArtifactId {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.splitn(2, ':').collect::<Vec<_>>();
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

/// Exhaustive artifact specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::runtime::ArtifactSpec")]
pub struct ArtifactSpec {
    /// Information uniquely identifying the artifact.
    pub artifact: ArtifactId,
    /// Runtime-specific artifact payload.
    pub payload: Vec<u8>,
}

impl ArtifactSpec {
    /// Generic constructor.
    pub fn new(artifact: ArtifactId, deploy_spec: impl BinaryValue) -> Self {
        Self {
            artifact,
            payload: deploy_spec.into_bytes(),
        }
    }
}

/// Exhaustive service instance specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::runtime::InstanceSpec")]
pub struct InstanceSpec {
    /// Unique numeric ID of the service instance.
    ///
    ///  Exonum assigns an ID to the service on instantiation. It is mainly used to route
    /// transaction messages belonging to this instance.
    pub id: InstanceId,
    /// Unique name of the service instance.
    ///
    /// The name serves as a primary identifier of this service in most operations.
    /// It is assigned by the network administrators.
    ///
    /// The name must correspond to the following regular expression: `[a-zA-Z0-9/\:-_]+`
    pub name: String,
    /// Identifier of the corresponding artifact.
    pub artifact: ArtifactId,
}

impl InstanceSpec {
    /// Create a new instance specification or return an error
    /// if the resulting specification is not correct.
    pub fn new(
        id: InstanceId,
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
            is_valid_index_name_component(name),
            "Service instance name ({}) contains illegal character, use only: a-zA-Z0-9 and one of _-", name
        );
        Ok(())
    }

    /// Return the corresponding descriptor of this instance specification.
    pub fn as_descriptor(&self) -> InstanceDescriptor<'_> {
        InstanceDescriptor {
            id: self.id,
            name: self.name.as_ref(),
        }
    }
}

impl ValidateInput for InstanceSpec {
    type Error = failure::Error;

    fn validate(&self) -> Result<(), Self::Error> {
        self.artifact.validate()?;
        Self::is_valid_name(&self.name)
    }
}

impl Display for InstanceSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{}", self.artifact, self.id, self.name)
    }
}

/// Allows to query a service instance by either of the two identifiers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstanceQuery<'a> {
    /// Query by an instance ID.
    Id(InstanceId),
    /// Query by an instance name.
    Name(&'a str),
}

impl From<InstanceId> for InstanceQuery<'_> {
    fn from(value: InstanceId) -> Self {
        InstanceQuery::Id(value)
    }
}

impl<'a> From<&'a str> for InstanceQuery<'a> {
    fn from(value: &'a str) -> Self {
        InstanceQuery::Name(value)
    }
}

/// Status of an artifact deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactStatus {
    /// The artifact is pending deployment.
    Pending = 0,
    /// The artifact has been successfully deployed.
    Active = 1,
}

impl Display for ArtifactStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactStatus::Active => f.write_str("active"),
            ArtifactStatus::Pending => f.write_str("pending"),
        }
    }
}

impl ProtobufConvert for ArtifactStatus {
    type ProtoStruct = schema::runtime::ArtifactStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        match self {
            ArtifactStatus::Active => schema::runtime::ArtifactStatus::ARTIFACT_ACTIVE,
            ArtifactStatus::Pending => schema::runtime::ArtifactStatus::ARTIFACT_PENDING,
        }
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        Ok(match pb {
            schema::runtime::ArtifactStatus::ARTIFACT_ACTIVE => ArtifactStatus::Active,
            schema::runtime::ArtifactStatus::ARTIFACT_PENDING => ArtifactStatus::Pending,
        })
    }
}

/// Status of a service instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InstanceStatus {
    /// The service instance is pending deployment.
    Pending = 0,
    /// The service instance has been successfully deployed.
    Active = 1,
}

impl Display for InstanceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstanceStatus::Active => f.write_str("active"),
            InstanceStatus::Pending => f.write_str("pending"),
        }
    }
}

impl ProtobufConvert for InstanceStatus {
    type ProtoStruct = schema::runtime::InstanceStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        match self {
            InstanceStatus::Active => schema::runtime::InstanceStatus::SERVICE_ACTIVE,
            InstanceStatus::Pending => schema::runtime::InstanceStatus::SERVICE_PENDING,
        }
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        Ok(match pb {
            schema::runtime::InstanceStatus::SERVICE_ACTIVE => InstanceStatus::Active,
            schema::runtime::InstanceStatus::SERVICE_PENDING => InstanceStatus::Pending,
        })
    }
}

/// Current state of artifact in dispatcher.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::runtime::ArtifactState")]
pub struct ArtifactState {
    /// Artifact specification.
    pub spec: ArtifactSpec,
    /// Artifact deployment status.
    pub status: ArtifactStatus,
}

impl ArtifactState {
    /// Create a new artifact state with the given specification and status.
    pub fn new(spec: ArtifactSpec, status: ArtifactStatus) -> Self {
        Self { spec, status }
    }
}

/// Current state of service instance in dispatcher.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::runtime::InstanceState")]
pub struct InstanceState {
    /// Service instance specification.
    pub spec: InstanceSpec,
    /// Service instance activity status.
    pub status: InstanceStatus,
}

impl InstanceState {
    /// Creates a new instance state with the given specification and status.
    pub fn new(spec: InstanceSpec, status: InstanceStatus) -> Self {
        Self { spec, status }
    }
}

#[test]
fn parse_artifact_id_correct() {
    "0:my-service:1.0.0".parse::<ArtifactId>().unwrap();
    "1:com.my.java.service.v1".parse::<ArtifactId>().unwrap();
}

#[test]
fn parse_artifact_id_incorrect_layout() {
    let artifacts = [
        ("15", "Wrong artifact id format"),
        ("0:", "Artifact name should not be empty"),
        (":", "cannot parse integer from empty string"),
        (":123", "cannot parse integer from empty string"),
        ("-1:123", "invalid digit found in string"),
        ("ava:123", "invalid digit found in string"),
        (
            "123:I am a service!",
            "Artifact name (I am a service!) contains an illegal character",
        ),
        (
            "123:\u{44e}\u{43d}\u{438}\u{43a}\u{43e}\u{434}\u{44b}!",
            "Artifact name (\u{44e}\u{43d}\u{438}\u{43a}\u{43e}\u{434}\u{44b}!) contains an illegal character",
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
    InstanceSpec::new(15, "foo-service", "0:my-service:1.0.0").unwrap();
}

#[test]
fn test_instance_spec_validate_incorrect() {
    let specs = [
        (
            InstanceSpec::new(1, "", "0:my-service:1.0.0"),
            "Service instance name should not be empty",
        ),
        (
            InstanceSpec::new(2,
                "\u{440}\u{443}\u{441}\u{441}\u{43a}\u{438}\u{439}_\u{441}\u{435}\u{440}\u{432}\u{438}\u{441}",
                "0:my-service:1.0.0"
            ),
            "Service instance name (\u{440}\u{443}\u{441}\u{441}\u{43a}\u{438}\u{439}_\u{441}\u{435}\u{440}\u{432}\u{438}\u{441}) contains illegal character",
        ),
        (
            InstanceSpec::new(3, "space service", "1:java.runtime.service"),
            "Service instance name (space service) contains illegal character",
        ),
        (
            InstanceSpec::new(4, "dot.service", "1:java.runtime.service"),
            "Service instance name (dot.service) contains illegal character",
        ),
        (
            InstanceSpec::new(5, "foo_service", ""),
            "Wrong artifact id format",
        ),
        (
            InstanceSpec::new(6, "foo_service", ":"),
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
