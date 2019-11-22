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

//! Internal dispatcher data types

use exonum_proto::ProtobufConvert;

use std::fmt;

use crate::{
    proto::schema,
    runtime::{ArtifactSpec, InstanceSpec},
};

/// Status of an artifact deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactStatus {
    /// The artifact is pending deployment.
    Pending = 0,
    /// The artifact has been successfully deployed.
    Active = 1,
}

impl fmt::Display for ArtifactStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => f.write_str("active"),
            Self::Pending => f.write_str("pending"),
        }
    }
}

impl ProtobufConvert for ArtifactStatus {
    type ProtoStruct = schema::dispatcher::ArtifactStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        match self {
            Self::Active => Self::ProtoStruct::ARTIFACT_ACTIVE,
            Self::Pending => Self::ProtoStruct::ARTIFACT_PENDING,
        }
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        Ok(match pb {
            Self::ProtoStruct::ARTIFACT_ACTIVE => Self::Active,
            Self::ProtoStruct::ARTIFACT_PENDING => Self::Pending,
        })
    }
}

/// Status of a service instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceStatus {
    /// The service instance is pending deployment.
    Pending = 0,
    /// The service instance has been successfully deployed.
    Active = 1,
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => f.write_str("active"),
            Self::Pending => f.write_str("pending"),
        }
    }
}

impl ProtobufConvert for ServiceStatus {
    type ProtoStruct = schema::dispatcher::ServiceStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        match self {
            Self::Active => Self::ProtoStruct::SERVICE_ACTIVE,
            Self::Pending => Self::ProtoStruct::SERVICE_PENDING,
        }
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        Ok(match pb {
            Self::ProtoStruct::SERVICE_ACTIVE => Self::Active,
            Self::ProtoStruct::SERVICE_PENDING => Self::Pending,
        })
    }
}

/// Current state of artifact in dispatcher.
#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::dispatcher::ArtifactState")]
pub struct ArtifactState {
    /// Artifact specification.
    pub spec: ArtifactSpec,
    /// Artifact deployment status.
    pub status: ArtifactStatus,
}

impl ArtifactState {
    /// Returns underlining artifact spec if status is active.
    pub fn active(self) -> Option<ArtifactSpec> {
        match self.status {
            ArtifactStatus::Active => Some(self.spec),
            ArtifactStatus::Pending => None,
        }
    }
}

impl From<ArtifactState> for (ArtifactStatus, ArtifactSpec) {
    fn from(v: ArtifactState) -> Self {
        (v.status, v.spec)
    }
}

/// Current state of service instance in dispatcher.
#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::dispatcher::InstanceState")]
pub struct InstanceState {
    /// Service instance specification.
    pub spec: InstanceSpec,
    /// Service instance activity status.
    pub status: ServiceStatus,
}

impl InstanceState {
    /// Returns underlining instance spec if status is active.
    pub fn active(self) -> Option<InstanceSpec> {
        match self.status {
            ServiceStatus::Active => Some(self.spec),
            ServiceStatus::Pending => None,
        }
    }
}

impl From<InstanceState> for (ServiceStatus, InstanceSpec) {
    fn from(v: InstanceState) -> Self {
        (v.status, v.spec)
    }
}
