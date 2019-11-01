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

use crate::{simple::SimpleSupervisorInterface, ConfigPropose};

/// Private API specification of the simple supervisor service.
pub trait PrivateApi {
    /// Error type for the current API implementation.
    type Error: Fail;

    /// Creates and broadcasts the `ConfigPropose` transaction, which is signed
    /// by the current node, and returns its hash.
    fn propose_config(&self, proposal: ConfigPropose) -> Result<Hash, Self::Error>;
}

struct ApiImpl<'a>(&'a ServiceApiState<'a>);

impl<'a> ApiImpl<'a> {
    fn broadcast_transaction(
        &self,
        transaction: impl Transaction<dyn SimpleSupervisorInterface>,
    ) -> Result<Hash, failure::Error> {
        let (pub_key, sec_key) = self.0.service_keypair;
        let signed = transaction.sign(self.0.instance.id, *pub_key, sec_key);

        let hash = signed.object_hash();
        self.0.sender().broadcast_transaction(signed)?;
        Ok(hash)
    }
}

impl PrivateApi for ApiImpl<'_> {
    type Error = api::Error;

    fn propose_config(&self, proposal: ConfigPropose) -> Result<Hash, Self::Error> {
        self.broadcast_transaction(proposal).map_err(From::from)
    }
}

pub fn wire(builder: &mut ServiceApiBuilder) {
    builder
        .private_scope()
        .endpoint_mut("propose-config", |state, query| {
            ApiImpl(state).propose_config(query)
        });
}
