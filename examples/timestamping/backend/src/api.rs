// Copyright 2018 The Exonum Team
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
    api::{self, ServiceApiBuilder, ServiceApiState}, blockchain::{self, BlockProof},
    crypto::{CryptoHash, Hash}, node::TransactionSend, storage::MapProof,
};

use schema::{Schema, TimestampEntry};
use transactions::TxTimestamp;
use TIMESTAMPING_SERVICE;

/// The structure describes the query parameters for `handle_timestamp`
/// and `handle_timestamp_proof` endpoints.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimestampQuery {
    /// The hash of the given timestamp.
    pub hash: Hash,
}

impl TimestampQuery {
    /// Creates new `TimestampQuery` with given `hash`.
    pub fn new(hash: Hash) -> Self {
        TimestampQuery { hash }
    }
}

/// The structure describes the information to prove the correctness of
/// timestamp entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampProof {
    /// The `BlockProof` of the appropriate block.
    pub block_info: BlockProof,
    /// The proof of the actual state of timestamping.
    pub state_proof: MapProof<Hash, Hash>,
    /// The proof of the appropriate `TimestampEntry`'s.
    pub timestamp_proof: MapProof<Hash, TimestampEntry>,
}

/// Public service API.
#[derive(Debug, Clone, Copy)]
pub struct PublicApi;

impl PublicApi {
    /// Endpoint for handling timestamping transaction.
    pub fn handle_post_transaction(
        state: &ServiceApiState,
        transaction: TxTimestamp,
    ) -> api::Result<Hash> {
        let hash = transaction.hash();
        state.sender().send(transaction.into())?;
        Ok(hash)
    }

    /// Endpoint for getting single timestamp.
    pub fn handle_timestamp(
        state: &ServiceApiState,
        query: TimestampQuery,
    ) -> api::Result<Option<TimestampEntry>> {
        let snapshot = state.snapshot();
        let schema = Schema::new(&snapshot);
        Ok(schema.timestamps().get(&query.hash))
    }

    /// Endpoint for getting the proof of single timestamp.
    pub fn handle_timestamp_proof(
        state: &ServiceApiState,
        query: TimestampQuery,
    ) -> api::Result<TimestampProof> {
        let snapshot = state.snapshot();
        let (state_proof, block_info) = {
            let core_schema = blockchain::Schema::new(&snapshot);
            let last_block_height = state.blockchain().last_block().height();
            let block_proof = core_schema.block_and_precommits(last_block_height).unwrap();
            let state_proof = core_schema.get_proof_to_service_table(TIMESTAMPING_SERVICE, 0);
            (state_proof, block_proof)
        };
        let schema = Schema::new(&snapshot);
        let timestamp_proof = schema.timestamps().get_proof(query.hash);
        Ok(TimestampProof {
            block_info,
            state_proof,
            timestamp_proof,
        })
    }

    /// Wires this endpoints to public scope of given `ServiceApiBuilder`.
    pub fn wire(builder: &mut ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("v1/timestamps/value", Self::handle_timestamp)
            .endpoint("v1/timestamps/proof", Self::handle_timestamp_proof)
            .endpoint_mut("v1/timestamps", Self::handle_post_transaction);
    }
}
