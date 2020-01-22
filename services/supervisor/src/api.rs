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

use exonum::{blockchain::ConsensusConfig, crypto::Hash, helpers::Height, runtime::ArtifactId};
use exonum_rust_runtime::{
    api::{self, ServiceApiBuilder, ServiceApiState},
    Broadcaster,
};
use failure::Fail;
use serde_derive::{Deserialize, Serialize};

use std::convert::TryFrom;

use super::{
    schema::SchemaImpl, transactions::SupervisorInterface, AsyncEventState, ConfigProposalWithHash,
    ConfigPropose, ConfigVote, DeployRequest, MigrationRequest, SupervisorConfig,
};

/// Query for retrieving information about deploy state.
/// This is flattened version of `DeployRequest` which can be
/// encoded via URL query parameters.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct DeployInfoQuery {
    /// Artifact identifier as string, e.g. `0:exonum-supervisor:0.13.0-rc.2"
    pub artifact: String,
    /// Artifact spec bytes as hexadecimal string.
    pub spec: String,
    /// Deadline height.
    pub deadline_height: u64,
}

impl TryFrom<DeployInfoQuery> for DeployRequest {
    type Error = api::Error;

    fn try_from(query: DeployInfoQuery) -> Result<Self, Self::Error> {
        let artifact = query
            .artifact
            .parse::<ArtifactId>()
            .map_err(|err| api::Error::BadRequest(err.to_string()))?;
        let spec =
            hex::decode(query.spec).map_err(|err| api::Error::BadRequest(err.to_string()))?;
        let deadline_height = Height(query.deadline_height);

        let request = Self {
            artifact,
            spec,
            deadline_height,
        };

        Ok(request)
    }
}

impl From<DeployRequest> for DeployInfoQuery {
    fn from(request: DeployRequest) -> Self {
        let artifact = request.artifact.to_string();
        let spec = hex::encode(&request.spec);
        let deadline_height = request.deadline_height.0;

        Self {
            artifact,
            spec,
            deadline_height,
        }
    }
}

/// Query for retrieving information about migration state.
/// This is flattened version of `MigrationRequest` which can be
/// encoded via URL query parameters.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct MigrationInfoQuery {
    /// Artifact identifier as string, e.g. `0:exonum-supervisor:0.13.0-rc.2"
    pub new_artifact: String,
    /// Target service name.
    pub service: String,
    /// Deadline height.
    pub deadline_height: u64,
}

impl TryFrom<MigrationInfoQuery> for MigrationRequest {
    type Error = api::Error;

    fn try_from(query: MigrationInfoQuery) -> Result<Self, Self::Error> {
        let new_artifact = query
            .new_artifact
            .parse::<ArtifactId>()
            .map_err(|err| api::Error::BadRequest(err.to_string()))?;
        let deadline_height = Height(query.deadline_height);

        let request = Self {
            new_artifact,
            service: query.service,
            deadline_height,
        };

        Ok(request)
    }
}

impl From<MigrationRequest> for MigrationInfoQuery {
    fn from(request: MigrationRequest) -> Self {
        let new_artifact = request.new_artifact.to_string();
        let deadline_height = request.deadline_height.0;

        Self {
            new_artifact,
            service: request.service,
            deadline_height,
        }
    }
}

/// Response with execution status for a certain asynchronous request.
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize)]
pub struct ProcessStateResponse {
    /// Process execution state. Can be `None` if there is no corresponding request.
    pub state: Option<AsyncEventState>,
}

impl ProcessStateResponse {
    /// Creates a new `ProcessStateResponse` object.
    pub fn new(state: Option<AsyncEventState>) -> Self {
        Self { state }
    }
}

/// Private API specification of the supervisor service.
pub trait PrivateApi {
    /// Error type for the current API implementation.
    type Error: Fail;

    /// Creates and broadcasts the `DeployArtifact` transaction, which is signed
    /// by the current node, and returns its hash.
    fn deploy_artifact(&self, artifact: DeployRequest) -> Result<Hash, Self::Error>;

    /// Creates and broadcasts the `MigrationRequest` transaction, which is signed
    /// by the current node, and returns its hash.
    fn migrate(&self, request: MigrationRequest) -> Result<Hash, Self::Error>;

    /// Creates and broadcasts the `ConfigPropose` transaction, which is signed
    /// by the current node, and returns its hash.
    fn propose_config(&self, proposal: ConfigPropose) -> Result<Hash, Self::Error>;

    /// Creates and broadcasts the `ConfigVote` transaction, which is signed
    /// by the current node, and returns its hash.
    fn confirm_config(&self, vote: ConfigVote) -> Result<Hash, Self::Error>;

    /// Returns the number of processed configurations.
    fn configuration_number(&self) -> Result<u64, Self::Error>;

    /// Returns an actual supervisor config.
    fn supervisor_config(&self) -> Result<SupervisorConfig, Self::Error>;

    /// Returns the state of deployment for the given deploy request.
    fn deploy_status(&self, request: DeployInfoQuery) -> Result<ProcessStateResponse, Self::Error>;

    /// Returns the state of migration for the given migration request.
    fn migration_status(
        &self,
        request: MigrationInfoQuery,
    ) -> Result<ProcessStateResponse, Self::Error>;
}

pub trait PublicApi {
    /// Error type for the current API implementation.
    type Error: Fail;
    /// Returns an actual consensus configuration of the blockchain.
    fn consensus_config(&self) -> Result<ConsensusConfig, Self::Error>;
    /// Returns an pending propose config change.
    fn config_proposal(&self) -> Result<Option<ConfigProposalWithHash>, Self::Error>;
}

struct ApiImpl<'a>(&'a ServiceApiState<'a>);

impl ApiImpl<'_> {
    fn broadcaster(&self) -> Result<Broadcaster<'_>, api::Error> {
        self.0
            .broadcaster()
            .ok_or_else(|| api::Error::BadRequest("Node is not a validator".to_owned()))
    }
}

impl PrivateApi for ApiImpl<'_> {
    type Error = api::Error;

    fn deploy_artifact(&self, artifact: DeployRequest) -> Result<Hash, Self::Error> {
        self.broadcaster()?
            .request_artifact_deploy((), artifact)
            .map_err(|e| api::Error::InternalError(e.into()))
    }

    fn migrate(&self, request: MigrationRequest) -> Result<Hash, Self::Error> {
        self.broadcaster()?
            .request_migration((), request)
            .map_err(|e| api::Error::InternalError(e.into()))
    }

    fn propose_config(&self, proposal: ConfigPropose) -> Result<Hash, Self::Error> {
        self.broadcaster()?
            .propose_config_change((), proposal)
            .map_err(|e| api::Error::InternalError(e.into()))
    }

    fn confirm_config(&self, vote: ConfigVote) -> Result<Hash, Self::Error> {
        self.broadcaster()?
            .confirm_config_change((), vote)
            .map_err(|e| api::Error::InternalError(e.into()))
    }

    fn configuration_number(&self) -> Result<u64, Self::Error> {
        let configuration_number =
            SchemaImpl::new(self.0.service_data()).get_configuration_number();
        Ok(configuration_number)
    }

    fn supervisor_config(&self) -> Result<SupervisorConfig, Self::Error> {
        let config = SchemaImpl::new(self.0.service_data()).supervisor_config();
        Ok(config)
    }

    fn deploy_status(&self, query: DeployInfoQuery) -> Result<ProcessStateResponse, Self::Error> {
        let request = DeployRequest::try_from(query)?;
        let schema = SchemaImpl::new(self.0.service_data());
        let status = schema.deploy_states.get(&request);

        Ok(ProcessStateResponse::new(status))
    }

    fn migration_status(
        &self,
        query: MigrationInfoQuery,
    ) -> Result<ProcessStateResponse, Self::Error> {
        let request = MigrationRequest::try_from(query)?;
        let schema = SchemaImpl::new(self.0.service_data());
        let status = schema
            .migration_states
            .get(&request)
            .map(|state| state.inner);

        Ok(ProcessStateResponse::new(status))
    }
}

impl PublicApi for ApiImpl<'_> {
    type Error = api::Error;

    fn consensus_config(&self) -> Result<ConsensusConfig, Self::Error> {
        Ok(self.0.data().for_core().consensus_config())
    }

    fn config_proposal(&self) -> Result<Option<ConfigProposalWithHash>, Self::Error> {
        Ok(SchemaImpl::new(self.0.service_data())
            .public
            .pending_proposal
            .get())
    }
}

pub fn wire(builder: &mut ServiceApiBuilder) {
    builder
        .private_scope()
        .endpoint_mut("deploy-artifact", |state, query| {
            ApiImpl(state).deploy_artifact(query)
        })
        .endpoint_mut("migrate", |state, query| ApiImpl(state).migrate(query))
        .endpoint_mut("propose-config", |state, query| {
            ApiImpl(state).propose_config(query)
        })
        .endpoint_mut("confirm-config", |state, query| {
            ApiImpl(state).confirm_config(query)
        })
        .endpoint("configuration-number", |state, _query: ()| {
            ApiImpl(state).configuration_number()
        })
        .endpoint("supervisor-config", |state, _query: ()| {
            ApiImpl(state).supervisor_config()
        })
        .endpoint("deploy-status", |state, query| {
            ApiImpl(state).deploy_status(query)
        })
        .endpoint("migration-status", |state, query| {
            ApiImpl(state).migration_status(query)
        });
    builder
        .public_scope()
        .endpoint("consensus-config", |state, _query: ()| {
            ApiImpl(state).consensus_config()
        })
        .endpoint("config-proposal", |state, _query: ()| {
            ApiImpl(state).config_proposal()
        });
}
