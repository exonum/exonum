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
    api::{self, ServiceApiBuilder, ServiceApiState},
    crypto::Hash,
    runtime::{
        rust::{ServiceDescriptor, Transaction},
        ServiceInstanceId,
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

struct ApiImpl<'a> {
    state: &'a ServiceApiState,
    instance_id: ServiceInstanceId,
    _instance_name: String,
}

impl<'a> ApiImpl<'a> {
    fn new(
        state: &'a ServiceApiState,
        instance_id: ServiceInstanceId,
        instance_name: &str,
    ) -> Self {
        Self {
            state,
            instance_id,
            _instance_name: instance_name.to_owned(),
        }
    }

    fn broadcast_transaction(&self, transaction: impl Transaction) -> Result<Hash, failure::Error> {
        let signed = transaction.sign(
            self.instance_id,
            *self.state.public_key(),
            self.state.secret_key(),
        );

        let hash = signed.object_hash();
        self.state.sender().broadcast_transaction(signed)?;
        Ok(hash)
    }
}

impl<'a> PrivateApi for ApiImpl<'a> {
    type Error = api::Error;

    fn deploy_artifact(&self, artifact: DeployRequest) -> Result<Hash, Self::Error> {
        self.broadcast_transaction(artifact).map_err(From::from)
    }

    fn start_service(&self, service: StartService) -> Result<Hash, Self::Error> {
        self.broadcast_transaction(service).map_err(From::from)
    }
}

pub fn wire(descriptor: &ServiceDescriptor, builder: &mut ServiceApiBuilder) {
    let instance_id = descriptor.service_id();
    let instance_name = descriptor.service_name().to_owned();

    builder
        .private_scope()
        .endpoint_mut("deploy-artifact", {
            let instance_name = instance_name.clone();
            move |state: &ServiceApiState, artifact: DeployRequest| {
                ApiImpl::new(state, instance_id, &instance_name).deploy_artifact(artifact)
            }
        })
        .endpoint_mut("start-service", {
            let instance_name = instance_name.clone();
            move |state: &ServiceApiState, service: StartService| {
                ApiImpl::new(state, instance_id, &instance_name).start_service(service)
            }
        });
}
