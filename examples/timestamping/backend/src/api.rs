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

//! REST API.

use exonum::{
    blockchain::{BlockProof, IndexProof},
    crypto::Hash,
};
use exonum_merkledb::{proof_map::Raw, MapProof};
use exonum_rust_runtime::api::{self, ServiceApiBuilder, ServiceApiState};

use crate::{
    schema::{Schema, TimestampEntry},
    transactions::Config,
};

/// Describes query parameters for `handle_timestamp` and `handle_timestamp_proof` endpoints.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimestampQuery {
    /// Hash of the requested timestamp.
    pub hash: Hash,
}

impl TimestampQuery {
    /// Creates new `TimestampQuery` with given `hash`.
    pub fn new(hash: Hash) -> Self {
        TimestampQuery { hash }
    }
}

/// Describes the information required to prove the correctness of the timestamp entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampProof {
    /// Proof of the last block.
    pub block_info: BlockProof,
    /// Actual state hashes of the timestamping service with their proofs.
    pub state_proof: MapProof<String, Hash>,
    /// Actual state of the timestamping database with proofs.
    pub timestamp_proof: MapProof<Hash, TimestampEntry, Raw>,
}

/// Public service API.
#[derive(Debug, Clone, Copy)]
pub struct PublicApi;

impl PublicApi {
    /// Endpoint for getting a single timestamp.
    pub fn handle_timestamp(
        self,
        state: &ServiceApiState<'_>,
        hash: &Hash,
    ) -> api::Result<Option<TimestampEntry>> {
        let schema = Schema::new(state.service_data());
        Ok(schema.timestamps.get(hash))
    }

    /// Endpoint for getting the proof of a single timestamp.
    pub fn handle_timestamp_proof(
        self,
        state: &ServiceApiState<'_>,
        hash: Hash,
    ) -> api::Result<TimestampProof> {
        let IndexProof {
            block_proof,
            index_proof,
            ..
        } = state.data().proof_for_service_index("timestamps").unwrap();

        let schema = Schema::new(state.service_data());
        let timestamp_proof = schema.timestamps.get_proof(hash);
        Ok(TimestampProof {
            block_info: block_proof,
            state_proof: index_proof,
            timestamp_proof,
        })
    }

    /// Endpoint for getting service configuration.
    pub fn get_service_configuration(self, state: &ServiceApiState<'_>) -> api::Result<Config> {
        Schema::new(state.service_data())
            .config
            .get()
            .ok_or_else(|| api::Error::not_found().title("Configuration not found"))
    }

    /// Wires the above endpoints to public API scope of the given `ServiceApiBuilder`.
    pub fn wire(self, builder: &mut ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("v1/timestamps/value", {
                move |state: &ServiceApiState<'_>, query: TimestampQuery| {
                    self.handle_timestamp(state, &query.hash)
                }
            })
            .endpoint("v1/timestamps/proof", {
                move |state: &ServiceApiState<'_>, query: TimestampQuery| {
                    self.handle_timestamp_proof(state, query.hash)
                }
            })
            .endpoint("v1/timestamps/config", {
                move |state: &ServiceApiState<'_>, _query: ()| self.get_service_configuration(state)
            });
    }
}
