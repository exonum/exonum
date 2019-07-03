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

pub use self::{
    errors::Error,
    proto::{DeployArtifact, StartService},
    schema::Schema,
};

use exonum_merkledb::Snapshot;

use crate::{
    api::ServiceApiBuilder,
    crypto::Hash,
    messages::ServiceInstanceId,
    runtime::rust::{RustArtifactId, Service, ServiceDescriptor, ServiceFactory},
};

mod api;
mod errors;
mod proto;
mod schema;
mod transactions;

#[derive(Debug)]
pub struct Supervisor;

impl Service for Supervisor {
    fn state_hash(&self, descriptor: ServiceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        Schema::new(descriptor.service_name(), snapshot).state_hash()
    }

    fn wire_api(&self, descriptor: ServiceDescriptor, builder: &mut ServiceApiBuilder) {
        api::wire(descriptor, builder)
    }
}

impl ServiceFactory for Supervisor {
    fn artifact(&self) -> RustArtifactId {
        RustArtifactId {
            name: "exonum-supervisor".to_owned(),
            version: env!("CARGO_PKG_VERSION").parse().unwrap(),
        }
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
    }
}

impl Supervisor {
    pub const BUILTIN_ID: ServiceInstanceId = 1;
    pub const BUILTIN_NAME: &'static str = "supervisor";
}
