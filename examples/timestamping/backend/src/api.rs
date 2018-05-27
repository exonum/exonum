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

use exonum::api::{Api, ApiError};
use exonum::blockchain::{self, BlockProof, Blockchain, Transaction};
use exonum::crypto::Hash;
use exonum::node::TransactionSend;
use exonum::storage::MapProof;

use bodyparser;
use iron::{IronResult, Plugin, Request, Response};
use router::Router;

use TIMESTAMPING_SERVICE;
use schema::{Schema, TimestampEntry};
use transactions::TxTimestamp;

#[derive(Debug, Serialize)]
pub struct TimestampProof {
    pub block_info: BlockProof,
    pub state_proof: MapProof<Hash, Hash>,
    pub timestamp_proof: MapProof<Hash, TimestampEntry>,
}

#[derive(Clone)]
pub struct PublicApi<T: TransactionSend + Clone + 'static> {
    channel: T,
    blockchain: Blockchain,
}

impl<T: TransactionSend + Clone + 'static> PublicApi<T> {
    pub fn new(blockchain: Blockchain, channel: T) -> PublicApi<T> {
        PublicApi {
            blockchain,
            channel,
        }
    }

    pub fn put_transaction<Tx: Transaction>(&self, tx: Tx) -> Result<Hash, ApiError> {
        let hash = tx.hash();
        self.channel.send(Box::new(tx))?;
        Ok(hash)
    }

    pub fn timestamp_proof(&self, content_hash: &Hash) -> Result<TimestampProof, ApiError> {
        let snapshot = self.blockchain.snapshot();
        let (state_proof, block_info) = {
            let core_schema = blockchain::Schema::new(&snapshot);
            let last_block_height = self.blockchain.last_block().height();
            let block_proof = core_schema.block_and_precommits(last_block_height).unwrap();
            let state_proof = core_schema.get_proof_to_service_table(TIMESTAMPING_SERVICE, 0);
            (state_proof, block_proof)
        };
        let schema = Schema::new(&snapshot);
        let timestamp_proof = schema.timestamps().get_proof(*content_hash);
        Ok(TimestampProof {
            block_info,
            state_proof,
            timestamp_proof,
        })
    }

    pub fn timestamp(&self, content_hash: &Hash) -> Result<Option<TimestampEntry>, ApiError> {
        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        Ok(schema.timestamps().get(content_hash))
    }

    fn wire_timestamp_proof(self, router: &mut Router) {
        let timestamp_proof = move |req: &mut Request| -> IronResult<Response> {
            let content_hash: Hash = self.url_fragment(req, "content_hash")?;
            let proof = self.timestamp_proof(&content_hash)?;
            self.ok_response(&json!(proof))
        };
        router.get(
            "/v1/timestamps/proof/:content_hash",
            timestamp_proof,
            "get_timestamp_proof",
        );
    }

    fn wire_timestamp(self, router: &mut Router) {
        let timestamp = move |req: &mut Request| -> IronResult<Response> {
            let content_hash: Hash = self.url_fragment(req, "content_hash")?;
            let timestamp = self.timestamp(&content_hash)?;
            self.ok_response(&json!(timestamp))
        };
        router.get(
            "/v1/timestamps/value/:content_hash",
            timestamp,
            "get_timestamp",
        );
    }

    fn wire_post_timestamp(self, router: &mut Router) {
        let post_timestamp = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<TxTimestamp>>() {
                Ok(Some(tx)) => {
                    let hash = self.put_transaction(tx)?;
                    self.ok_response(&json!(hash))
                }
                Ok(None) => Err(ApiError::BadRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::BadRequest(e.to_string()))?,
            }
        };
        router.post("/v1/timestamps", post_timestamp, "post_timestamp");
    }
}

impl<T: TransactionSend + Clone + 'static> Api for PublicApi<T> {
    fn wire(&self, router: &mut Router) {
        self.clone().wire_timestamp(router);
        self.clone().wire_timestamp_proof(router);
        self.clone().wire_post_timestamp(router);
    }
}
