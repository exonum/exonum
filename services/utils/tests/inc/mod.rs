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

//! Simple service for testing.

use exonum::{
    crypto::{Hash, PublicKey},
    merkledb::{access::Access, BinaryValue, MapIndex, Snapshot},
    runtime::{
        rust::{ArtifactProtobufSpec, CallContext, Service, ServiceFactory},
        ArtifactId, BlockchainData, DispatcherError, ExecutionError, RuntimeIdentifier,
    },
};
use exonum_derive::*;
use failure::ensure;
use semver::Version;

use std::borrow::Cow;

#[derive(Debug, Clone, Copy, ObjectHash)]
pub struct Inc {
    seed: u8,
}

impl Inc {
    pub fn new(seed: u8) -> Self {
        Self { seed }
    }
}

impl BinaryValue for Inc {
    fn to_bytes(&self) -> Vec<u8> {
        vec![self.seed]
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, failure::Error> {
        ensure!(bytes.len() == 1, "Invalid `Inc` size");
        Ok(Self { seed: bytes[0] })
    }
}

#[derive(Debug, FromAccess)]
pub struct IncSchema<T: Access> {
    pub counts: MapIndex<T::Base, PublicKey, u64>,
}

#[exonum_interface]
pub trait IncInterface {
    fn increment(&self, context: CallContext<'_>, arg: Inc) -> Result<(), ExecutionError>;
}

#[derive(Clone, Default, Debug, ServiceDispatcher)]
#[service_dispatcher(implements("IncInterface"))]
pub struct IncService;

impl Service for IncService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

impl IncInterface for IncService {
    fn increment(&self, context: CallContext<'_>, _arg: Inc) -> Result<(), ExecutionError> {
        let author = context
            .caller()
            .author()
            .ok_or(DispatcherError::UnauthorizedCaller)?;
        let mut schema = IncSchema::new(context.service_data());
        let count = schema.counts.get(&author).unwrap_or_default();
        schema.counts.put(&author, count + 1);
        Ok(())
    }
}

#[derive(Debug)]
pub struct IncFactory {
    version: Version,
}

impl IncFactory {
    pub const ARTIFACT_NAME: &'static str = "exonum.services.test.Inc";

    pub fn new(version: Version) -> Self {
        Self { version }
    }
}

impl ServiceFactory for IncFactory {
    fn artifact_id(&self) -> ArtifactId {
        ArtifactId {
            runtime_id: RuntimeIdentifier::Rust as _,
            name: Self::ARTIFACT_NAME.to_owned(),
            version: self.version.clone(),
        }
    }

    fn artifact_protobuf_spec(&self) -> ArtifactProtobufSpec {
        ArtifactProtobufSpec::default()
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(IncService)
    }
}
