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

use exonum::{
    crypto::Hash,
    runtime::{
        api::{self, ServiceApiBuilder, ServiceApiState},
        rust::Transaction,
    },
};
use exonum_merkledb::ObjectHash;
use failure::Fail;

use super::{
    schema::Schema, ConfigProposalWithHash, ConfigPropose, ConfigVote, DeployRequest, StartService,
};
use exonum::blockchain::{ConsensusConfig, Schema as CoreSchema};

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
    /// Creates and broadcasts the `ConfigPropose` transaction, which is signed
    /// by the current node, and returns its hash.
    fn propose_config(&self, proposal: ConfigPropose) -> Result<Hash, Self::Error>;
    /// Creates and broadcasts the `ConfigVote` transaction, which is signed
    /// by the current node, and returns its hash.
    fn confirm_config(&self, vote: ConfigVote) -> Result<Hash, Self::Error>;
}

pub trait PublicApi {
    /// Error type for the current API implementation.
    type Error: Fail;
    /// Returns an actual consensus configuration of the blockchain.
    fn consensus_config(&self) -> Result<ConsensusConfig, Self::Error>;
    /// Returns an pending propose config change.
    fn config_proposal(&self) -> Result<Vec<ConfigProposalWithHash>, Self::Error>;
}

struct ApiImpl<'a>(&'a ServiceApiState<'a>);

impl<'a> ApiImpl<'a> {
    fn broadcast_transaction(&self, transaction: impl Transaction) -> Result<Hash, failure::Error> {
        let keypair = self.0.service_keypair;
        let signed = transaction.sign(self.0.instance.id, keypair.0, &keypair.1);

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

    fn propose_config(&self, proposal: ConfigPropose) -> Result<Hash, Self::Error> {
        // Discard proposes whose `actual from` heights are same with already registered proposes
        if Schema::new(self.0.instance.name,self.0.snapshot())
            .pending_propose_hashes().contains(&proposal.actual_from.0) {
            Err(Self::Error::from(failure::format_err!("Config proposal with the same height already registered")))
        } else {
            self.broadcast_transaction(proposal).map_err(From::from)
        }
    }

    fn confirm_config(&self, vote: ConfigVote) -> Result<Hash, Self::Error> {
        self.broadcast_transaction(vote).map_err(From::from)
    }
}

impl PublicApi for ApiImpl<'_> {
    type Error = api::Error;

    fn consensus_config(&self) -> Result<ConsensusConfig, Self::Error> {
        Ok(CoreSchema::new(self.0.snapshot()).consensus_config())
    }

    fn config_proposal(&self) -> Result<Vec<ConfigProposalWithHash>, Self::Error> {
        Ok(Schema::new(self.0.instance.name, self.0.snapshot())
            .pending_propose_hashes().values()
            .collect())
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
        })
        .endpoint_mut("propose-config", |state, query| {
            ApiImpl(state).propose_config(query)
        })
        .endpoint_mut("confirm-config", |state, query| {
            ApiImpl(state).confirm_config(query)
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
