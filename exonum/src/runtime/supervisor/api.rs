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

use exonum_merkledb::ObjectHash;
use failure::Fail;

use super::{DeployRequest, StartService};
use crate::{
    crypto::Hash,
    runtime::{
        api::{self, ServiceApiBuilder, ServiceApiState},
        rust::Transaction,
    },
};

/// Private API specification of the supervisor service.
pub trait PrivateApi {
    /// Error type for the current API implementation.
    type Error: Fail;
    /// Creates and broadcasts the `DeployArtifact` transaction, which is signed
    /// by the current node, and returns its hash.
    fn deploy_artifact(&self, artifact: DeployRequest) -> Result<Hash, Self::Error>;
    /// Creates and broadcasts the `StartService` transaction, which is signed
    /// by the current node, and returns its hash.
    fn start_service(&self, service: StartService) -> Result<Hash, Self::Error>;
}

struct ApiImpl<'a>(&'a ServiceApiState<'a>);

impl<'a> ApiImpl<'a> {
    fn broadcast_transaction(&self, transaction: impl Transaction) -> Result<Hash, failure::Error> {
        let keypair = self.0.service_keypair();
        let signed = transaction.sign(self.0.instance().id, *keypair.0, keypair.1);

        let hash = signed.object_hash();
        self.0.sender().broadcast_transaction(signed)?;
        Ok(hash)
    }
}

impl PrivateApi for ApiImpl<'_> {
    type Error = api::Error;

    fn deploy_artifact(&self, artifact: DeployRequest) -> Result<Hash, Self::Error> {
        self.broadcast_transaction(artifact).map_err(From::from)
    }

    fn start_service(&self, service: StartService) -> Result<Hash, Self::Error> {
        self.broadcast_transaction(service).map_err(From::from)
    }
}

pub fn wire(builder: &mut ServiceApiBuilder) {
    builder
        .private_scope()
        .endpoint_mut("deploy-artifact", |state, query| {
            ApiImpl(state).deploy_artifact(query)
        })
        .endpoint_mut("start-service", |state, query| {
            ApiImpl(state).start_service(query)
        });
}
