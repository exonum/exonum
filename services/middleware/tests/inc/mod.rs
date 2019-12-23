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
    crypto::PublicKey,
    merkledb::{access::Access, MapIndex},
    runtime::{ArtifactId, DispatcherError, ExecutionError, RuntimeIdentifier},
};
use exonum_derive::*;
use exonum_rust_runtime::{ArtifactProtobufSpec, CallContext, Service, ServiceFactory};
use semver::Version;

use exonum_middleware_service::ArtifactReq;

#[derive(Debug, FromAccess)]
pub struct IncSchema<T: Access> {
    pub counts: MapIndex<T::Base, PublicKey, u64>,
}

#[exonum_interface]
pub trait IncInterface<Ctx> {
    type Output;
    fn increment(&self, context: Ctx, seed: u8) -> Self::Output;
}

#[derive(Clone, Default, Debug, ServiceDispatcher)]
#[service_dispatcher(implements("IncInterface"))]
pub struct IncService;

impl Service for IncService {}

impl IncInterface<CallContext<'_>> for IncService {
    type Output = Result<(), ExecutionError>;

    fn increment(&self, context: CallContext<'_>, _seed: u8) -> Self::Output {
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

#[derive(Debug, Clone)]
pub struct IncFactory {
    version: Version,
}

impl IncFactory {
    pub const ARTIFACT_NAME: &'static str = "exonum.services.test.Inc";

    pub fn new(version: Version) -> Self {
        Self { version }
    }

    pub fn req(version_req: &str) -> ArtifactReq {
        ArtifactReq {
            name: Self::ARTIFACT_NAME.to_owned(),
            version: version_req
                .parse()
                .expect("Cannot parse version requirement"),
        }
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
