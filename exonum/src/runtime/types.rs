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

use anyhow::{bail, ensure, format_err};
use exonum_crypto::{Hash, KeyPair, PublicKey, SecretKey, HASH_SIZE};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_merkledb::{
    impl_binary_key_for_binary_value,
    indexes::proof_map::RawKey,
    validation::{is_allowed_index_name_char, is_valid_index_name_component},
    BinaryKey, BinaryValue, ObjectHash,
};
use exonum_proto::ProtobufConvert;
use protobuf::well_known_types::Empty;
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
#[protobuf_convert(source = "schema::base::CallInfo")]
#[non_exhaustive]
pub struct CallInfo {
    /// Unique service instance identifier. The dispatcher uses this identifier to find the
    /// runtime to execute a transaction.
    pub instance_id: InstanceId,
    /// Identifier of the method in the service interface required for the call.
    pub method_id: MethodId,
}

impl CallInfo {
    /// Creates a `CallInfo` instance.
    pub fn new(instance_id: u32, method_id: u32) -> Self {
        Self {
            instance_id,
            method_id,
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
/// let keypair = crypto::KeyPair::random();
/// // Service instance which we want to call.
/// let instance_id = 1024;
/// // Specific method of the service interface.
/// let method_id = 0;
/// let call_info = CallInfo::new(instance_id, method_id);
///
/// // `AnyTx` object created from `CallInfo` and payload.
/// let arguments = "Talk is cheap. Show me the code. – Linus Torvalds".to_owned().into_bytes();
/// let any_tx = AnyTx::new(call_info, arguments);
/// let transaction = any_tx.sign_with_keypair(&keypair);
/// ```
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "schema::base::AnyTx")]
#[non_exhaustive]
pub struct AnyTx {
    /// Information required for the call of the corresponding executor.
    pub call_info: CallInfo,
    /// Serialized transaction arguments.
    pub arguments: Vec<u8>,
}

impl AnyTx {
    /// Creates a new `AnyTx` object.
    pub fn new(call_info: CallInfo, arguments: Vec<u8>) -> Self {
        Self {
            call_info,
            arguments,
        }
    }

    /// Signs a transaction with the specified Ed25519 keys.
    pub fn sign(self, public_key: PublicKey, secret_key: &SecretKey) -> Verified<Self> {
        Verified::from_value(self, public_key, secret_key)
    }

    /// Signs a transaction with the specified Ed25519 keypair.
    pub fn sign_with_keypair(self, keypair: &KeyPair) -> Verified<Self> {
        Verified::from_value(self, keypair.public_key(), keypair.secret_key())
    }

    /// Parse transaction arguments as a specific type.
    pub fn parse<T: BinaryValue>(&self) -> anyhow::Result<T> {
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
/// # fn main() -> anyhow::Result<()> {
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
#[protobuf_convert(source = "schema::base::ArtifactId")]
#[non_exhaustive]
pub struct ArtifactId {
    /// Runtime identifier.
    pub runtime_id: u32,
    /// Artifact name.
    pub name: String,
    /// Semantic version of the artifact.
    #[protobuf_convert(with = "crate::helpers::pb_version")]
    pub version: Version,
}

#[allow(clippy::needless_pass_by_value)] // required for work with `protobuf_convert(with)`
impl ArtifactId {
    /// Creates a new artifact identifier from the given runtime id and name
    /// or returns error if the resulting artifact id is not correct.
    pub fn new(
        runtime_id: impl Into<u32>,
        name: impl Into<String>,
        version: Version,
    ) -> anyhow::Result<Self> {
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
    type Error = anyhow::Error;

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
            "Artifact name contains an illegal character, use only: `a-zA-Z0-9` and `/_.-`"
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
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.splitn(3, ':').collect::<Vec<_>>();
        match &split[..] {
            [runtime_id, name, version] => {
                let artifact = Self::new(
                    u32::from_str(runtime_id)?,
                    (*name).to_string(),
                    version.parse()?,
                )?;
                artifact.validate()?;
                Ok(artifact)
            }
            _ => Err(anyhow::format_err!(
                "Wrong `ArtifactId` format, should be in form \"runtime_id:name:version\""
            )),
        }
    }
}

/// Exhaustive artifact specification. This information is enough to deploy an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::base::ArtifactSpec")]
#[non_exhaustive]
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
#[protobuf_convert(source = "schema::base::InstanceSpec")]
#[non_exhaustive]
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
    /// The name must correspond to the following regular expression: `[a-zA-Z0-9/\:-_]+`.
    pub name: String,

    /// Identifier of the corresponding artifact.
    pub artifact: ArtifactId,
}

impl InstanceSpec {
    /// Creates a new instance specification or return an error
    /// if the resulting specification is not correct.
    pub fn new(
        id: InstanceId,
        name: impl Into<String>,
        artifact: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        let spec = Self::from_raw_parts(id, name.into(), artifact.as_ref().parse()?);
        spec.validate()?;
        Ok(spec)
    }

    /// Creates a new instance specification from prepared parts without any checks.
    pub fn from_raw_parts(id: InstanceId, name: String, artifact: ArtifactId) -> Self {
        Self { id, name, artifact }
    }

    /// Checks that the instance name contains only allowed characters and is not empty.
    pub fn is_valid_name(name: impl AsRef<str>) -> anyhow::Result<()> {
        let name = name.as_ref();
        ensure!(!name.is_empty(), "Service name is empty");
        ensure!(
            is_valid_index_name_component(name),
            "Service name contains illegal character, use only: a-zA-Z0-9 and _-"
        );
        Ok(())
    }

    /// Returns the corresponding descriptor of this instance specification.
    pub fn as_descriptor(&self) -> InstanceDescriptor {
        InstanceDescriptor::new(self.id, &self.name)
    }
}

impl ValidateInput for InstanceSpec {
    type Error = anyhow::Error;

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
#[non_exhaustive]
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
#[non_exhaustive]
pub enum ArtifactStatus {
    /// The artifact is pending unload.
    Unloading,
    /// The artifact is pending deployment.
    Deploying,
    /// The artifact has been successfully deployed.
    Active,
}

impl Display for ArtifactStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unloading => f.write_str("unloading"),
            Self::Active => f.write_str("active"),
            Self::Deploying => f.write_str("deploying"),
        }
    }
}

impl ProtobufConvert for ArtifactStatus {
    type ProtoStruct = schema::lifecycle::ArtifactState_Status;

    fn to_pb(&self) -> Self::ProtoStruct {
        use self::schema::lifecycle::ArtifactState_Status::*;

        match self {
            Self::Unloading => UNLOADING,
            Self::Active => ACTIVE,
            Self::Deploying => DEPLOYING,
        }
    }

    fn from_pb(pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        use self::schema::lifecycle::ArtifactState_Status::*;

        Ok(match pb {
            UNLOADING => Self::Unloading,
            ACTIVE => Self::Active,
            DEPLOYING => Self::Deploying,
        })
    }
}

/// Information about a migration of a service instance.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "schema::lifecycle::InstanceMigration")]
#[non_exhaustive]
pub struct InstanceMigration {
    /// Migration target to obtain migration scripts from. This artifact
    /// must be deployed on the blockchain.
    pub target: ArtifactId,

    /// Version of the instance data after the migration is completed.
    /// Note that it does not necessarily match the version of `target`,
    /// but should be not greater.
    #[protobuf_convert(with = "crate::helpers::pb_version")]
    pub end_version: Version,

    /// Consensus-wide outcome of the migration, in the form of the aggregation hash
    /// of the migrated data. The lack of value signifies that the network has not yet reached
    /// consensus about the migration outcome.
    #[protobuf_convert(with = "crate::helpers::pb_optional_hash")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_hash: Option<Hash>,
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
        }
    }

    /// Checks if the migration is considered completed, i.e., has migration state agreed
    /// among all nodes in the blockchain network.
    pub fn is_completed(&self) -> bool {
        self.completed_hash.is_some()
    }
}

/// Status of a service instance.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[derive(BinaryValue)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum InstanceStatus {
    /// The service instance is active.
    Active,
    /// The service instance is stopped.
    Stopped,
    /// The service instance is frozen; it can process read-only requests,
    /// but not transactions and `before_transactions` / `after_transactions` hooks.
    Frozen,
    /// The service instance is migrating to the specified artifact.
    Migrating(Box<InstanceMigration>),
}

impl InstanceStatus {
    pub(super) fn migrating(migration: InstanceMigration) -> Self {
        Self::Migrating(Box::new(migration))
    }

    /// Indicates whether the service instance status is active.
    pub fn is_active(&self) -> bool {
        *self == Self::Active
    }

    /// Returns `true` if a service with this status provides at least read access to its data.
    pub fn provides_read_access(&self) -> bool {
        match self {
            // Migrations are non-destructive currently; i.e., the old service data is consistent
            // during migration.
            Self::Active | Self::Frozen | Self::Migrating(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if the service instance with this status can be resumed.
    pub fn can_be_resumed(&self) -> bool {
        match self {
            Self::Stopped | Self::Frozen => true,
            _ => false,
        }
    }

    /// Returns `true` if the service instance with this status can be stopped.
    pub fn can_be_stopped(&self) -> bool {
        match self {
            Self::Active | Self::Frozen => true,
            _ => false,
        }
    }

    /// Returns `true` if the service instance with this status can be frozen in all cases.
    pub fn can_be_frozen(&self) -> bool {
        match self {
            Self::Active => true,
            // We cannot easily transition `Stopped` -> `Frozen` because a `Stopped` service
            // may have a data version differing from the artifact recorded in service spec,
            // or, more generally, from any of deployed artifacts.
            _ => false,
        }
    }

    pub(super) fn ongoing_migration_target(&self) -> Option<&ArtifactId> {
        match self {
            Self::Migrating(migration) if !migration.is_completed() => Some(&migration.target),
            _ => None,
        }
    }

    pub(super) fn completed_migration_hash(&self) -> Option<Hash> {
        match self {
            Self::Migrating(migration) => migration.completed_hash,
            _ => None,
        }
    }
}

impl Display for InstanceStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Active => "active",
            Self::Stopped => "stopped",
            Self::Frozen => "frozen",
            Self::Migrating(..) => "migrating",
        })
    }
}

impl InstanceStatus {
    // Used by `InstanceState`.
    #[allow(clippy::wrong_self_convention)]
    pub(super) fn to_pb(status: &Option<Self>) -> schema::lifecycle::InstanceStatus {
        Self::create_pb(status.as_ref())
    }

    fn create_pb(status: Option<&Self>) -> schema::lifecycle::InstanceStatus {
        use schema::lifecycle::InstanceStatus_Simple::*;

        let mut pb = schema::lifecycle::InstanceStatus::new();
        match status {
            None => pb.set_simple(NONE),
            Some(Self::Active) => pb.set_simple(ACTIVE),
            Some(Self::Stopped) => pb.set_simple(STOPPED),
            Some(Self::Frozen) => pb.set_simple(FROZEN),
            Some(Self::Migrating(migration)) => pb.set_migration(migration.to_pb()),
        }
        pb
    }

    pub(super) fn from_pb(
        mut pb: schema::lifecycle::InstanceStatus,
    ) -> anyhow::Result<Option<Self>> {
        use schema::lifecycle::InstanceStatus_Simple::*;

        if pb.has_simple() {
            Ok(match pb.get_simple() {
                NONE => None,
                ACTIVE => Some(Self::Active),
                STOPPED => Some(Self::Stopped),
                FROZEN => Some(Self::Frozen),
            })
        } else if pb.has_migration() {
            InstanceMigration::from_pb(pb.take_migration())
                .map(|migration| Some(Self::migrating(migration)))
        } else {
            Err(format_err!("No variant specified for `InstanceStatus`"))
        }
    }
}

impl ProtobufConvert for InstanceStatus {
    type ProtoStruct = schema::lifecycle::InstanceStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        Self::create_pb(Some(self))
    }

    fn from_pb(pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        let maybe_self = Self::from_pb(pb)?;
        maybe_self
            .ok_or_else(|| format_err!("Cannot create `InstanceStatus` from `None` serialization"))
    }
}

/// Current state of an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::lifecycle::ArtifactState")]
#[non_exhaustive]
pub struct ArtifactState {
    /// Runtime-specific deployment specification.
    pub deploy_spec: Vec<u8>,
    /// Artifact deployment status.
    pub status: ArtifactStatus,
}

impl ArtifactState {
    /// Creates an artifact state with the given specification and status.
    pub(super) fn new(deploy_spec: Vec<u8>, status: ArtifactStatus) -> Self {
        Self {
            deploy_spec,
            status,
        }
    }
}

/// Current state of service instance in dispatcher.
#[derive(Debug, Clone, PartialEq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::lifecycle::InstanceState")]
#[non_exhaustive]
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
    ///
    /// Status can be `None` only during the block execution if instance was created,
    /// but activation routine for it is not yet completed, and this value can occur no more
    /// than once in a service lifetime.
    ///
    /// If this field is set to `None`, the pending_status must have value
    /// `Some(InstanceStatus::Active)`.
    #[protobuf_convert(with = "InstanceStatus")]
    pub status: Option<InstanceStatus>,

    /// Pending status of the instance.
    ///
    /// Pending state can be not `None` if core is in process of changing service status,
    /// e.g. service initialization, resuming or migration. If this field was set to value
    /// other than `None`, it always will be reset to `None` in the next block.
    ///
    /// The purpose of this field is to keep information about further service status during the
    /// block execution because the service status can be changed only after that block is
    /// committed. This approach is needed because there is no guarantee that the executed
    /// block will be committed.
    #[protobuf_convert(with = "InstanceStatus")]
    pub pending_status: Option<InstanceStatus>,
}

mod pb_optional_version {
    use super::*;

    #[allow(clippy::needless_pass_by_value)] // required for work with `protobuf_convert(with)`
    pub fn from_pb(pb: String) -> anyhow::Result<Option<Version>> {
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

    /// Returns the artifact currently associated with the service; that is, one that understands
    /// its data and is deployed on the blockchain.
    ///
    /// This method will return `None` if a service has been [migrated] because the migration
    /// workflow does not guarantee that the resulting data version corresponds to a deployed
    /// artifact.
    ///
    /// A [runtime] may use this method to determine how to treat service state updates.
    ///
    /// [migrated]: migrations/index.html
    /// [runtime]: trait.Runtime.html
    pub fn associated_artifact(&self) -> Option<&ArtifactId> {
        if self.data_version.is_some() {
            None
        } else {
            Some(&self.spec.artifact)
        }
    }

    /// Returns true if a service with this state can have its data read.
    pub(super) fn is_readable(&self) -> bool {
        let status = self
            .status
            .as_ref()
            .or_else(|| self.pending_status.as_ref());
        status.map_or(false, InstanceStatus::provides_read_access)
    }

    /// Sets next status as current and changes next status to `None`
    ///
    /// # Panics
    ///
    /// - If next status is already `None`.
    pub(super) fn commit_pending_status(&mut self) {
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

impl From<Result<Hash, String>> for MigrationStatus {
    fn from(res: Result<Hash, String>) -> Self {
        Self(res)
    }
}

impl ProtobufConvert for MigrationStatus {
    type ProtoStruct = schema::lifecycle::MigrationStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        match self.0 {
            Ok(hash) => pb.set_hash(hash.to_pb()),
            Err(ref e) => pb.set_error(e.clone()),
        }
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        let inner = if pb.has_hash() {
            Ok(Hash::from_pb(pb.take_hash())?)
        } else if pb.has_error() {
            Err(pb.take_error())
        } else {
            return Err(format_err!(
                "Invalid Protobuf for `MigrationStatus`: neither of variants is specified"
            ));
        };
        Ok(Self(inner))
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
#[derive(Debug, PartialEq, Clone)]
#[derive(BinaryValue, ObjectHash)]
#[non_exhaustive]
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
}

impl Caller {
    /// Returns the author's public key, if it exists.
    pub fn author(&self) -> Option<PublicKey> {
        if let Self::Transaction { author } = self {
            Some(*author)
        } else {
            None
        }
    }

    /// Tries to reinterpret the caller as a service.
    pub fn as_service(&self) -> Option<InstanceId> {
        if let Self::Service { instance_id } = self {
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
    type ProtoStruct = schema::auth::Caller;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        match self {
            Self::Transaction { author } => pb.set_transaction_author(author.to_pb()),
            Self::Service { instance_id } => pb.set_instance_id(*instance_id),
            Self::Blockchain => pb.set_blockchain(Empty::new()),
        }
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        Ok(if pb.has_transaction_author() {
            let author = PublicKey::from_pb(pb.take_transaction_author())?;
            Self::Transaction { author }
        } else if pb.has_instance_id() {
            Self::Service {
                instance_id: pb.get_instance_id(),
            }
        } else if pb.has_blockchain() {
            Self::Blockchain
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
/// let public_key = crypto::KeyPair::random().public_key();
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

    fn from_pb(pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        Hash::from_pb(pb).map(Self)
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
        Self(Hash::read(buffer))
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
                "Artifact name contains an illegal character",
            ),
            (
                "123:\u{44e}\u{43d}\u{438}\u{43a}\u{43e}\u{434}\u{44b}:1.0.0",
                "Artifact name contains an illegal character",
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
                "Service name is empty",
            ),
            (
                InstanceSpec::new(2,
                    "\u{440}\u{443}\u{441}\u{441}\u{43a}\u{438}\u{439}_\u{441}\u{435}\u{440}\u{432}\u{438}\u{441}",
                    "0:my-service:1.0.0"
                ),
                "Service name contains illegal character",
            ),
            (
                InstanceSpec::new(3, "space service", "1:java.runtime.service:1.0.0"),
                "Service name contains illegal character",
            ),
            (
                InstanceSpec::new(4, "foo_service", ""),
                "Wrong `ArtifactId` format",
            ),
            (
                InstanceSpec::new(5, "dot.service", "1:java.runtime.service:1.0.0"),
                "Service name contains illegal character",
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
