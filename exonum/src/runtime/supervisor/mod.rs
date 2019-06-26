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

pub use self::{proto::{DeployArtifact, StartService}, schema::Schema};

use crate::runtime::rust::{RustArtifactId, Service, ServiceFactory};

mod proto;
mod schema;
mod transactions;

#[derive(Debug)]
pub struct Supervisor;

impl Service for Supervisor {}

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
