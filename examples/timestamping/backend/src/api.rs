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

//! REST API.
use exonum_merkledb::MapProof;

use exonum::{
    blockchain::{self, BlockProof, IndexCoordinates, IndexOwner},
    crypto::Hash,
    runtime::api::{self, ServiceApiBuilder, ServiceApiState},
};

use crate::schema::{Schema, TimestampEntry};

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
    pub state_proof: MapProof<IndexCoordinates, Hash>,
    /// Actual state of the timestamping database with proofs.
    pub timestamp_proof: MapProof<Hash, TimestampEntry>,
}

/// Public service API.
#[derive(Debug, Clone, Copy)]
pub struct PublicApi;

impl PublicApi {
    /// Endpoint for getting a single timestamp.
    pub fn handle_timestamp(
        self,
        state: &ServiceApiState,
        hash: &Hash,
    ) -> api::Result<Option<TimestampEntry>> {
        let snapshot = state.snapshot();
        let schema = Schema::new(state.instance.name, snapshot);
        Ok(schema.timestamps().get(hash))
    }

    /// Endpoint for getting the proof of a single timestamp.
    pub fn handle_timestamp_proof(
        self,
        state: &ServiceApiState,
        hash: Hash,
    ) -> api::Result<TimestampProof> {
        let snapshot = state.snapshot();
        let (state_proof, block_info) = {
            let blockchain_schema = blockchain::Schema::new(snapshot);
            let last_block_height = blockchain_schema.last_block().height();
            let block_proof = blockchain_schema
                .block_and_precommits(last_block_height)
                .unwrap();
            let state_proof = blockchain_schema
                .state_hash_aggregator()
                .get_proof(IndexOwner::Service(state.instance.id).coordinate_for(0));
            (state_proof, block_proof)
        };
        let schema = Schema::new(state.instance.name, snapshot);
        let timestamp_proof = schema.timestamps().get_proof(hash);
        Ok(TimestampProof {
            block_info,
            state_proof,
            timestamp_proof,
        })
    }

    /// Wires the above endpoints to public API scope of the given `ServiceApiBuilder`.
    pub fn wire(self, builder: &mut ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("v1/timestamps/value", {
                move |state: &ServiceApiState, query: TimestampQuery| {
                    self.handle_timestamp(state, &query.hash)
                }
            })
            .endpoint("v1/timestamps/proof", {
                move |state: &ServiceApiState, query: TimestampQuery| {
                    self.handle_timestamp_proof(state, query.hash)
                }
            });
    }
}
