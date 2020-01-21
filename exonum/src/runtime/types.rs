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

use exonum_crypto::{Hash, PublicKey, SecretKey, HASH_SIZE};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_merkledb::{
    impl_binary_key_for_binary_value,
    indexes::proof_map::RawKey,
    validation::{is_allowed_index_name_char, is_valid_index_name_component},
    BinaryKey, BinaryValue, ObjectHash,
};
use exonum_proto::ProtobufConvert;
use failure::{bail, ensure, format_err};
use semver::Version;
use serde_derive::{Deserialize, Serialize};

use std::{
    borrow::Cow,
    fmt::{self, Display},
    str::FromStr,
};

use super::InstanceDescriptor;
use crate::{
    blockchain::config::InstanceInitParams, helpers::ValidateInput, messages::Verified,
    proto::schema,
};

/// Unique service instance identifier.
///
/// * This is a secondary identifier, mainly used in transaction messages.
/// The primary one is a service instance name.
///
/// * The dispatcher assigns this identifier when the service is started.
pub type InstanceId = u32;
/// Identifier of the method in the service interface required for the call.
pub type MethodId = u32;

/// Information sufficient to route a transaction to a service.
#[derive(Default, Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert)]
#[protobuf_convert(source = "schema::runtime::CallInfo")]
pub struct CallInfo {
    /// Unique service instance identifier. The dispatcher uses this identifier to find the
    /// runtime to execute a transaction.
    pub instance_id: InstanceId,
    /// Identifier of the method in the service interface required for the call.
    pub method_id: MethodId,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl CallInfo {
    /// Creates a `CallInfo` instance.
    pub fn new(instance_id: u32, method_id: u32) -> Self {
        Self {
            instance_id,
            method_id,
            non_exhaustive: (),
        }
    }
}

/// Transaction with the information required to dispatch it to a service.
///
/// # Examples
///
/// Creates a new signed transaction.
///
/// ```
/// use exonum::{
///     crypto,
///     messages::Verified,
///     runtime::{AnyTx, CallInfo},
/// };
///
/// let keypair = crypto::gen_keypair();
/// // Service instance which we want to call.
/// let instance_id = 1024;
/// // Specific method of the service interface.
/// let method_id = 0;
/// let call_info = CallInfo::new(instance_id, method_id);
///
/// // `AnyTx` object created from `CallInfo` and payload.
/// let arguments = "Talk is cheap. Show me the code. – Linus Torvalds".to_owned().into_bytes();
/// let any_tx = AnyTx::new(call_info, arguments);
///
/// let transaction = Verified::from_value(
///     any_tx,
///     keypair.0,
///     &keypair.1
/// );
/// ```
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "schema::runtime::AnyTx")]
pub struct AnyTx {
    /// Information required for the call of the corresponding executor.
    pub call_info: CallInfo,
    /// Serialized transaction arguments.
    pub arguments: Vec<u8>,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl AnyTx {
    /// Creates a new `AnyTx` object.
    pub fn new(call_info: CallInfo, arguments: Vec<u8>) -> Self {
        Self {
            call_info,
            arguments,
            non_exhaustive: (),
        }
    }

    /// Signs a transaction with the specified Ed25519 keypair.
    pub fn sign(self, public_key: PublicKey, secret_key: &SecretKey) -> Verified<Self> {
        Verified::from_value(self, public_key, secret_key)
    }

    /// Parse transaction arguments as a specific type.
    pub fn parse<T: BinaryValue>(&self) -> Result<T, failure::Error> {
        T::from_bytes(Cow::Borrowed(&self.arguments))
    }
}

/// The artifact identifier is required to construct service instances.
/// In other words, an artifact identifier is similar to a class name, and a specific service
/// instance is similar to a class instance.
///
/// An artifact ID has the following string representation:
///
/// ```text
/// {runtime_id}:{artifact_name}:{version}
/// ```
///
/// where:
///
/// - `runtime_id` is a [runtime identifier],
/// - `artifact_name` is the name of the artifact
/// - `version` is the artifact semantic version
///
/// Artifact name may contain the following characters: `a-zA-Z0-9` and `_.-`.
///
/// [runtime identifier]: enum.RuntimeIdentifier.html
///
/// # Examples
///
/// ```
/// # use exonum::runtime::ArtifactId;
/// # fn main() -> Result<(), failure::Error> {
/// // Typical Rust artifact.
/// let rust_artifact_id = "0:my-service:1.0.0".parse::<ArtifactId>()?;
/// // Typical Java artifact.
/// let java_artifact_id = "1:com.exonum.service:1.0.0".parse::<ArtifactId>()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash, ProtobufConvert)]
#[protobuf_convert(source = "schema::runtime::ArtifactId")]
pub struct ArtifactId {
    /// Runtime identifier.
    pub runtime_id: u32,
    /// Artifact name.
    pub name: String,
    /// Semantic version of the artifact.
    #[protobuf_convert(with = "self::pb_version")]
    pub version: Version,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

mod pb_version {
    use super::*;

    #[allow(clippy::needless_pass_by_value)] // required for work with `protobuf_convert(with)`
    pub fn from_pb(pb: String) -> Result<Version, failure::Error> {
        pb.parse().map_err(From::from)
    }

    pub fn to_pb(value: &Version) -> String {
        value.to_string()
    }
}

impl ArtifactId {
    /// Creates a new artifact identifier from the given runtime id and name
    /// or returns error if the resulting artifact id is not correct.
    pub fn new(
        runtime_id: impl Into<u32>,
        name: impl Into<String>,
        version: Version,
    ) -> Result<Self, failure::Error> {
        let artifact = Self::from_raw_parts(runtime_id.into(), name.into(), version);
        artifact.validate()?;
        Ok(artifact)
    }

    /// Creates a new artifact identifier from prepared parts without any checks.
    ///
    /// Use this method only if you don't need an artifact verification (e.g. in tests).
    ///
    /// # Stability
    ///
    /// Since the internal structure of `ArtifactId` can change, this method is considered
    /// unstable and can break in the future.
    pub fn from_raw_parts(runtime_id: u32, name: String, version: Version) -> Self {
        Self {
            runtime_id,
            name,
            version,
            non_exhaustive: (),
        }
    }

    /// Checks if the specified artifact is an upgraded version of another artifact.
    pub fn is_upgrade_of(&self, other: &Self) -> bool {
        self.name == other.name && self.version > other.version
    }

    /// Converts into `InstanceInitParams` with the given IDs and an empty constructor.
    pub fn into_default_instance(
        self,
        id: InstanceId,
        name: impl Into<String>,
    ) -> InstanceInitParams {
        InstanceInitParams::new(id, name, self, ())
    }
}

impl ValidateInput for ArtifactId {
    type Error = failure::Error;

    /// Checks that the artifact name contains only allowed characters and is not empty.
    fn validate(&self) -> Result<(), Self::Error> {
        // This function is similar to `is_valid_identifier` from `merkledb`, but also
        // allows `/` to be a part of artifact name.
        fn is_valid_identifier(name: &str) -> bool {
            name.as_bytes()
                .iter()
                .all(|&c| is_allowed_index_name_char(c) || c == b'.' || c == b'/')
        }

        ensure!(!self.name.is_empty(), "Artifact name should not be empty");
        ensure!(
            is_valid_identifier(&self.name),
            "Artifact name ({}) contains an illegal character, use only: `a-zA-Z0-9` and `/_.-`",
            &self.name,
        );
        Ok(())
    }
}

impl_binary_key_for_binary_value! { ArtifactId }

impl Display for ArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.runtime_id, self.name, self.version
        )
    }
}

impl FromStr for ArtifactId {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.splitn(3, ':').collect::<Vec<_>>();
        match &split[..] {
            [runtime_id, name, version] => {
                let artifact = Self::new(
                    u32::from_str(runtime_id)?,
                    name.to_string(),
                    version.parse()?,
                )?;
                artifact.validate()?;
                Ok(artifact)
            }
            _ => Err(failure::format_err!(
                "Wrong `ArtifactId` format, should be in form \"runtime_id:name:version\""
            )),
        }
    }
}

/// Exhaustive artifact specification. This information is enough to deploy an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::runtime::ArtifactSpec")]
pub struct ArtifactSpec {
    /// Information uniquely identifying the artifact.
    pub artifact: ArtifactId,
    /// Runtime-specific artifact payload.
    pub payload: Vec<u8>,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl ArtifactSpec {
    /// Generic constructor.
    pub fn new(artifact: ArtifactId, deploy_spec: impl BinaryValue) -> Self {
        Self {
            artifact,
            payload: deploy_spec.into_bytes(),
            non_exhaustive: (),
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
    /// Exonum assigns an ID to the service on instantiation. It is mainly used to route
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

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl InstanceSpec {
    /// Creates a new instance specification or return an error
    /// if the resulting specification is not correct.
    pub fn new(
        id: InstanceId,
        name: impl Into<String>,
        artifact: impl AsRef<str>,
    ) -> Result<Self, failure::Error> {
        let spec = Self::from_raw_parts(id, name.into(), artifact.as_ref().parse()?);
        spec.validate()?;
        Ok(spec)
    }

    /// Creates a new instance specification from prepared parts without any checks.
    pub fn from_raw_parts(id: InstanceId, name: String, artifact: ArtifactId) -> Self {
        Self {
            id,
            name,
            artifact,
            non_exhaustive: (),
        }
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
        InstanceDescriptor::new(self.id, self.name.as_ref())
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
///
/// This type is not intended to be exhaustively matched. It can be extended in the future
/// without breaking the semver compatibility.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstanceQuery<'a> {
    /// Query by an instance ID.
    Id(InstanceId),
    /// Query by an instance name.
    Name(&'a str),

    /// Never actually generated.
    #[doc(hidden)]
    __NonExhaustive,
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
///
/// This type is not intended to be exhaustively matched. It can be extended in the future
/// without breaking the semver compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactStatus {
    /// The artifact is pending deployment.
    Pending = 1,
    /// The artifact has been successfully deployed.
    Active = 2,

    /// Never actually generated.
    #[doc(hidden)]
    __NonExhaustive,
}

impl Display for ArtifactStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactStatus::Active => f.write_str("active"),
            ArtifactStatus::Pending => f.write_str("pending"),
            ArtifactStatus::__NonExhaustive => unreachable!("Never actually generated"),
        }
    }
}

impl ProtobufConvert for ArtifactStatus {
    type ProtoStruct = schema::runtime::ArtifactState_Status;

    fn to_pb(&self) -> Self::ProtoStruct {
        match self {
            ArtifactStatus::Active => schema::runtime::ArtifactState_Status::ACTIVE,
            ArtifactStatus::Pending => schema::runtime::ArtifactState_Status::PENDING,
            ArtifactStatus::__NonExhaustive => unreachable!("Never actually generated"),
        }
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        Ok(match pb {
            schema::runtime::ArtifactState_Status::ACTIVE => ArtifactStatus::Active,
            schema::runtime::ArtifactState_Status::PENDING => ArtifactStatus::Pending,
            schema::runtime::ArtifactState_Status::NONE => {
                bail!("Status `NONE` is reserved for the further usage.")
            }
        })
    }
}

/// Information about a migration of a service instance.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "schema::runtime::InstanceMigration")]
pub struct InstanceMigration {
    /// Migration target to obtain migration scripts from. This artifact
    /// must be deployed on the blockchain.
    pub target: ArtifactId,

    /// Version of the instance data after the migration is completed.
    /// Note that it does not necessarily match the version of `target`,
    /// but should be not greater.
    #[protobuf_convert(with = "self::pb_version")]
    pub end_version: Version,

    /// Consensus-wide outcome of the migration, in the form of the aggregation hash
    /// of the migrated data. The lack of value signifies that the network has not yet reached
    /// consensus about the migration outcome.
    #[protobuf_convert(with = "self::pb_optional_hash")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_hash: Option<Hash>,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl InstanceMigration {
    pub(super) fn new(target: ArtifactId, end_version: Version) -> Self {
        Self::from_raw_parts(target, end_version, None)
    }

    pub(super) fn from_raw_parts(
        target: ArtifactId,
        end_version: Version,
        completed_hash: Option<Hash>,
    ) -> Self {
        Self {
            target,
            end_version,
            completed_hash,
            non_exhaustive: (),
        }
    }

    /// Checks if the migration is considered completed, i.e., has migration state agreed
    /// among all nodes in the blockchain network.
    pub fn is_completed(&self) -> bool {
        self.completed_hash.is_some()
    }
}

mod pb_optional_hash {
    use super::*;
    use exonum_crypto::proto::types::Hash as PbHash;

    pub fn from_pb(pb: PbHash) -> Result<Option<Hash>, failure::Error> {
        if pb.get_data().is_empty() {
            Ok(None)
        } else {
            Hash::from_pb(pb).map(Some)
        }
    }

    pub fn to_pb(value: &Option<Hash>) -> PbHash {
        if let Some(hash) = value {
            hash.to_pb()
        } else {
            PbHash::new()
        }
    }
}

/// Status of a service instance.
///
/// This type is not intended to be exhaustively matched. It can be extended in the future
/// without breaking the semver compatibility.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[derive(BinaryValue)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstanceStatus {
    /// The service instance is active.
    Active,
    /// The service instance is stopped.
    Stopped,
    /// The service instance is migrating to the specified artifact.
    Migrating(Box<InstanceMigration>),

    /// Never actually generated.
    #[doc(hidden)]
    __NonExhaustive,
}

impl InstanceStatus {
    pub(super) fn migrating(migration: InstanceMigration) -> Self {
        InstanceStatus::Migrating(Box::new(migration))
    }

    /// Indicates whether the service instance status is active.
    pub fn is_active(&self) -> bool {
        *self == InstanceStatus::Active
    }

    pub(super) fn ongoing_migration_target(&self) -> Option<&ArtifactId> {
        match self {
            InstanceStatus::Migrating(migration) if !migration.is_completed() => {
                Some(&migration.target)
            }
            _ => None,
        }
    }

    pub(super) fn completed_migration_hash(&self) -> Option<Hash> {
        match self {
            InstanceStatus::Migrating(migration) => migration.completed_hash,
            _ => None,
        }
    }
}

impl Display for InstanceStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            InstanceStatus::Active => "active",
            InstanceStatus::Stopped => "stopped",
            InstanceStatus::Migrating(..) => "migrating",
            InstanceStatus::__NonExhaustive => unreachable!("Never actually constructed"),
        })
    }
}

impl InstanceStatus {
    // Used by `InstanceState`.
    #[allow(clippy::wrong_self_convention)]
    pub(super) fn to_pb(status: &Option<Self>) -> schema::runtime::InstanceStatus {
        Self::create_pb(status.as_ref())
    }

    fn create_pb(status: Option<&Self>) -> schema::runtime::InstanceStatus {
        use schema::runtime::InstanceStatus_Simple::*;

        let mut pb = schema::runtime::InstanceStatus::new();
        match status {
            None => pb.set_simple(NONE),
            Some(InstanceStatus::Active) => pb.set_simple(ACTIVE),
            Some(InstanceStatus::Stopped) => pb.set_simple(STOPPED),
            Some(InstanceStatus::Migrating(migration)) => pb.set_migration(migration.to_pb()),
            Some(InstanceStatus::__NonExhaustive) => unreachable!("Never actually constructed"),
        }
        pb
    }

    pub(super) fn from_pb(
        mut pb: schema::runtime::InstanceStatus,
    ) -> Result<Option<Self>, failure::Error> {
        use schema::runtime::InstanceStatus_Simple::*;

        if pb.has_simple() {
            Ok(match pb.get_simple() {
                NONE => None,
                ACTIVE => Some(InstanceStatus::Active),
                STOPPED => Some(InstanceStatus::Stopped),
            })
        } else if pb.has_migration() {
            InstanceMigration::from_pb(pb.take_migration())
                .map(|migration| Some(InstanceStatus::migrating(migration)))
        } else {
            Err(format_err!("No variant specified for `InstanceStatus`"))
        }
    }
}

impl ProtobufConvert for InstanceStatus {
    type ProtoStruct = schema::runtime::InstanceStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        Self::create_pb(Some(self))
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let maybe_self = Self::from_pb(pb)?;
        maybe_self
            .ok_or_else(|| format_err!("Cannot create `InstanceStatus` from `None` serialization"))
    }
}

/// Current state of an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::runtime::ArtifactState")]
pub struct ArtifactState {
    /// Runtime-specific deployment specification.
    pub deploy_spec: Vec<u8>,
    /// Artifact deployment status.
    pub status: ArtifactStatus,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl ArtifactState {
    /// Creates an artifact state with the given specification and status.
    pub(super) fn new(deploy_spec: Vec<u8>, status: ArtifactStatus) -> Self {
        Self {
            deploy_spec,
            status,
            non_exhaustive: (),
        }
    }
}

/// Current state of service instance in dispatcher.
#[derive(Debug, Clone, PartialEq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::runtime::InstanceState")]
pub struct InstanceState {
    /// Service instance specification.
    pub spec: InstanceSpec,

    /// Version of the service data. `None` value means that the data version is the same
    /// as the `spec.artifact`. `Some(version)` means that one or more [data migrations] have
    /// been performed on the service, so that the service data is compatible with the `version`
    /// of the artifact.
    ///
    /// [data migrations]: migrations/index.html
    #[protobuf_convert(with = "self::pb_optional_version")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_version: Option<Version>,

    /// Service instance activity status.
    #[protobuf_convert(with = "InstanceStatus")]
    pub status: Option<InstanceStatus>,

    /// Pending status of instance if the value is not `None`.
    #[protobuf_convert(with = "InstanceStatus")]
    pub pending_status: Option<InstanceStatus>,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

mod pb_optional_version {
    use super::*;

    #[allow(clippy::needless_pass_by_value)] // required for work with `protobuf_convert(with)`
    pub fn from_pb(pb: String) -> Result<Option<Version>, failure::Error> {
        if pb.is_empty() {
            Ok(None)
        } else {
            pb.parse().map(Some).map_err(From::from)
        }
    }

    pub fn to_pb(value: &Option<Version>) -> String {
        if let Some(value) = value.as_ref() {
            value.to_string()
        } else {
            String::new()
        }
    }
}

impl InstanceState {
    pub(crate) fn from_raw_parts(
        spec: InstanceSpec,
        data_version: Option<Version>,
        status: Option<InstanceStatus>,
        pending_status: Option<InstanceStatus>,
    ) -> Self {
        Self {
            spec,
            data_version,
            status,
            pending_status,
            non_exhaustive: (),
        }
    }

    /// Returns the version of the service data. This can match the version of the service artifact,
    /// or may be greater if [data migrations] have been performed on the service.
    ///
    /// [data migrations]: migrations/index.html
    pub fn data_version(&self) -> &Version {
        self.data_version
            .as_ref()
            .unwrap_or(&self.spec.artifact.version)
    }

    /// Sets next status as current and changes next status to `None`
    ///
    /// # Panics
    ///
    /// - If next status is already `None`.
    pub(crate) fn commit_pending_status(&mut self) {
        assert!(
            self.pending_status.is_some(),
            "Next instance status should not be `None`"
        );
        self.status = self.pending_status.take();
    }
}

/// Result of execution of a migration script.
#[derive(Debug, Clone)]
#[derive(BinaryValue, ObjectHash)]
pub struct MigrationStatus(pub Result<Hash, String>);

impl ProtobufConvert for MigrationStatus {
    type ProtoStruct = schema::runtime::MigrationStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        match self.0 {
            Ok(hash) => pb.set_hash(hash.to_pb()),
            Err(ref e) => pb.set_error(e.clone()),
        }
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let inner = if pb.has_hash() {
            Ok(Hash::from_pb(pb.take_hash())?)
        } else if pb.has_error() {
            Err(pb.take_error())
        } else {
            return Err(format_err!(
                "Invalid Protobuf for `MigrationStatus`: neither of variants is specified"
            ));
        };
        Ok(MigrationStatus(inner))
    }
}

/// The authorization information for a service call.
///
/// `Caller` provides authorization details about the call. The called service may use `Caller`
/// to decide whether to proceed with the processing, or to return an error because the caller
/// has insufficient privileges. In some other cases (e.g., crypto-tokens), `Caller` may be used
/// to get or modify information about the caller in the blockchain state (e.g., the current token
/// balance).
///
/// Authorization info is not purely determined by the call stack. While outermost
/// transactions calls always have `Transaction` auth, services may make internal
/// calls, which either inherit the parent authorization or authorize a child call in their
/// name (`Service` auth). This is decided by the service; both kinds of auth may make sense
/// depending on the use case. Inherited auth makes sense for "middleware" (e.g.,
/// batched calls), while service auth makes sense for stateful authorization (e.g.,
/// multi-signatures).
///
/// Note that `Caller` has a forward-compatible uniform representation obtained via
/// [`address()`](#method.address) method. Services may use this representation to compare
/// or index callers without the necessity to care about all possible kinds of authorization
/// supported by the framework.
///
/// This enum is not supposed to be exhaustively matched, so that new variants may be added to it
/// without breaking semver compatibility.
#[derive(Debug, PartialEq, Clone)]
#[derive(BinaryValue, ObjectHash)]
pub enum Caller {
    /// A usual transaction from the Exonum client authorized by its key pair.
    Transaction {
        /// Public key of the user who signed this transaction.
        author: PublicKey,
    },

    /// The call is invoked with the authority of a blockchain service.
    Service {
        /// Identifier of the service instance which invoked the call.
        instance_id: InstanceId,
    },

    /// The call is invoked by one of the blockchain lifecycle events.
    ///
    /// This kind of authorization is used for `before_transactions` / `after_transactions`
    /// calls to the service instances, and for initialization of the built-in services.
    Blockchain,

    // Hidden variant to prevent exhaustive matching.
    #[doc(hidden)]
    __NonExhaustive,
}

impl Caller {
    /// Returns the author's public key, if it exists.
    pub fn author(&self) -> Option<PublicKey> {
        if let Caller::Transaction { author } = self {
            Some(*author)
        } else {
            None
        }
    }

    /// Tries to reinterpret the caller as a service.
    pub fn as_service(&self) -> Option<InstanceId> {
        if let Caller::Service { instance_id } = self {
            Some(*instance_id)
        } else {
            None
        }
    }

    /// Verifies that the caller of this method is a supervisor service.
    pub fn as_supervisor(&self) -> Option<()> {
        self.as_service().and_then(|instance_id| {
            if instance_id == super::SUPERVISOR_INSTANCE_ID {
                Some(())
            } else {
                None
            }
        })
    }

    /// Returns a uniform, forward-compatible presentation of the `Caller` that can be used
    /// as the account *address*. Different addresses are guaranteed to correspond to
    /// different `Caller`s.
    pub fn address(&self) -> CallerAddress {
        CallerAddress(self.object_hash())
    }
}

impl ProtobufConvert for Caller {
    type ProtoStruct = schema::runtime::Caller;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        match self {
            Caller::Transaction { author } => pb.set_transaction_author(author.to_pb()),
            Caller::Service { instance_id } => pb.set_instance_id(*instance_id),
            Caller::Blockchain => pb.set_blockchain(Default::default()),
            Caller::__NonExhaustive => unreachable!("variant is never constructed"),
        }
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        Ok(if pb.has_transaction_author() {
            let author = PublicKey::from_pb(pb.take_transaction_author())?;
            Caller::Transaction { author }
        } else if pb.has_instance_id() {
            Caller::Service {
                instance_id: pb.get_instance_id(),
            }
        } else if pb.has_blockchain() {
            Caller::Blockchain
        } else {
            bail!("No variant specified for `Caller`");
        })
    }
}

/// Uniform presentation of a `Caller`.
///
/// # Converting to Address
///
/// The address for a [`Caller`] is defined as the SHA-256 digest of its Protobuf serialization.
/// This ensures that addresses are unique, collision-resistant and domain-separated for different
/// `Caller` types.
///
/// For example, to compute an address from a public key, you can use `CallerAddress::from_key()`
/// (in Rust code), or create and hash a `Caller` Protobuf message (in any programming language).
///
/// ```
/// # use exonum::{crypto, merkledb::BinaryValue, runtime::{Caller, CallerAddress}};
/// let (public_key, _) = crypto::gen_keypair();
/// let address = CallerAddress::from_key(public_key);
/// let caller = Caller::Transaction { author: public_key };
/// // Obtain Protobuf serialization of the `Caller`.
/// let caller_pb = caller.to_bytes();
/// assert_eq!(address.as_ref(), &crypto::hash(&caller_pb)[..]);
/// ```
///
/// [`Caller`]: enum.Caller.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[serde(transparent)]
pub struct CallerAddress(Hash);

impl CallerAddress {
    /// Converts a public key to an address.
    pub fn from_key(public_key: PublicKey) -> Self {
        Caller::Transaction { author: public_key }.address()
    }
}

impl AsRef<[u8]> for CallerAddress {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl ProtobufConvert for CallerAddress {
    type ProtoStruct = exonum_crypto::proto::types::Hash;

    fn to_pb(&self) -> Self::ProtoStruct {
        self.0.to_pb()
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        Hash::from_pb(pb).map(CallerAddress)
    }
}

impl BinaryKey for CallerAddress {
    fn size(&self) -> usize {
        self.0.size()
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        self.0.write(buffer)
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        CallerAddress(Hash::read(buffer))
    }
}

// SAFETY: We proxy `Hash` implementation of raw keys which satisfies expected invariants.
#[allow(unsafe_code)]
unsafe impl RawKey for CallerAddress {
    fn to_raw_key(&self) -> [u8; HASH_SIZE] {
        self.0.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;
    use exonum_crypto as crypto;

    #[test]
    fn parse_artifact_id_correct() {
        let artifact_id = "0:my-service:1.0.0".parse::<ArtifactId>().unwrap();
        assert_eq!(artifact_id.runtime_id, 0);
        assert_eq!(artifact_id.name, "my-service");
        assert_eq!(artifact_id.version, Version::new(1, 0, 0));

        let artifact_id = "1:com.my.java.service:3.1.5-beta.2"
            .parse::<ArtifactId>()
            .unwrap();
        assert_eq!(artifact_id.runtime_id, 1);
        assert_eq!(artifact_id.name, "com.my.java.service");
        assert_eq!(artifact_id.version.major, 3);
        assert_eq!(artifact_id.version.minor, 1);
        assert_eq!(artifact_id.version.patch, 5);

        let artifact_id = "0:my-service/additional:1.0.0"
            .parse::<ArtifactId>()
            .unwrap();
        assert_eq!(artifact_id.runtime_id, 0);
        assert_eq!(artifact_id.name, "my-service/additional");
        assert_eq!(artifact_id.version, Version::new(1, 0, 0));
    }

    #[test]
    fn artifact_id_in_json() {
        let artifact_id = "0:my-service:1.0.0".parse::<ArtifactId>().unwrap();
        assert_eq!(
            serde_json::to_value(artifact_id).unwrap(),
            json!({
                "runtime_id": 0,
                "name": "my-service",
                "version": "1.0.0",
            })
        );

        let artifact_id = "0:my-service:2.0.0-rc.3".parse::<ArtifactId>().unwrap();
        assert_eq!(
            serde_json::to_value(artifact_id).unwrap(),
            json!({
                "runtime_id": 0,
                "name": "my-service",
                "version": "2.0.0-rc.3",
            })
        );
    }

    #[test]
    fn parse_artifact_id_incorrect_layout() {
        let artifacts = [
            ("15", "Wrong `ArtifactId` format"),
            ("0::3.1.0", "Artifact name should not be empty"),
            (":test:1.0.0", "cannot parse integer from empty string"),
            ("-1:test:1.0.0", "invalid digit found in string"),
            ("ava:test:0.0.1", "invalid digit found in string"),
            (
                "123:I am a service!:1.0.0",
                "Artifact name (I am a service!) contains an illegal character",
            ),
            (
                "123:\u{44e}\u{43d}\u{438}\u{43a}\u{43e}\u{434}\u{44b}:1.0.0",
                "Artifact name (\u{44e}\u{43d}\u{438}\u{43a}\u{43e}\u{434}\u{44b}) contains an illegal character",
            ),
            ("1:test:1", "Expected dot"),
            ("1:test:3.141593", "Expected dot"),
            ("1:test:what_are_versions", "Error parsing major identifier"),
            ("1:test:1.x.0", "Error parsing minor identifier"),
            ("1:test:1.0.x", "Error parsing patch identifier"),
            ("1:test:1.0.0:garbage", "Extra junk after valid version"),
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
                InstanceSpec::new(3, "space service", "1:java.runtime.service:1.0.0"),
                "Service instance name (space service) contains illegal character",
            ),
            (
                InstanceSpec::new(4, "foo_service", ""),
                "Wrong `ArtifactId` format",
            ),
            (
                InstanceSpec::new(5, "dot.service", "1:java.runtime.service:1.0.0"),
                "Service instance name (dot.service) contains illegal character",
            ),
            (
                InstanceSpec::new(6, "foo_service", ":test:1.0.0"),
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

    /// As per Protobuf spec, `Caller` serialization used to compute `address` contains
    /// at least the tag, thus providing domain separation.
    #[test]
    fn caller_addresses() {
        let blockchain_addr = Caller::Blockchain.address();
        let supervisor_addr = Caller::Service { instance_id: 0 }.address();
        assert_ne!(blockchain_addr.0, crypto::hash(&[]));
        assert_ne!(supervisor_addr.0, crypto::hash(&[]));
        assert_ne!(blockchain_addr, supervisor_addr);
    }
}
