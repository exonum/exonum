// Copyright 2017 The Exonum Team
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

use std::time::SystemTime;
use std::collections::HashMap;

use router::Router;
use iron::prelude::*;

use node::state::TxPool;
use blockchain::{Blockchain, Schema, SharedNodeState};
use crypto::{PublicKey, Hash};
use explorer::{BlockchainExplorer, TxInfo};
use api::{Api, ApiError};
use encoding::serialize::FromHex;
use helpers::Height;

#[derive(Serialize)]
struct MemPoolTxInfo {
    content: ::serde_json::Value,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum MemPoolResult {
    Unknown,
    MemPool(MemPoolTxInfo),
    Committed(TxInfo),
}

#[derive(Serialize)]
struct MemPoolInfo {
    size: usize,
}

#[derive(Serialize, Clone, Debug)]
struct PrecommitInfo {
    height: Height,
    timestamp: SystemTime,
}

#[derive(Serialize, Clone, Debug)]
pub struct HealthCheckInfo {
    height: Height,

    service_key: PublicKey,
    consensus_key: Option<PublicKey>,

    precommits: Vec<PrecommitInfo>,

    /// All validators incl. current
    chain: HashMap<PublicKey, Height>,
}

/// Public system API.
#[derive(Clone, Debug)]
pub struct SystemApi {
    pool: TxPool,
    blockchain: Blockchain,
    shared_api_state: SharedNodeState,
}

impl SystemApi {
    /// Creates a new `private::SystemApi` instance.
    pub fn new(
        pool: TxPool,
        blockchain: Blockchain,
        shared_api_state: SharedNodeState,
    ) -> SystemApi {
        SystemApi {
            pool,
            blockchain,
            shared_api_state,
        }
    }

    fn get_mempool_info(&self) -> MemPoolInfo {
        MemPoolInfo { size: self.pool.read().expect("Expected read lock").len() }
    }

    fn get_transaction(&self, hash_str: &str) -> Result<MemPoolResult, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        self.pool
            .read()
            .expect("Expected read lock")
            .get(&hash)
            .map_or_else(
                || {
                    let explorer = BlockchainExplorer::new(&self.blockchain);
                    Ok(explorer.tx_info(&hash)?.map_or(
                        MemPoolResult::Unknown,
                        MemPoolResult::Committed,
                    ))
                },
                |o| Ok(MemPoolResult::MemPool(MemPoolTxInfo { content: o.info() })),
            )
    }

    fn get_healthcheck_info(&self) -> HealthCheckInfo {
        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(snapshot);
        let conf = schema.actual_configuration();

        let height = schema.height();
        let service_key = *self.blockchain.api_context().public_key();
        let consensus_key = conf
            .validator_keys
            .iter()
            .find(|key| service_key == key.service_key)
            .map(|key| key.consensus_key);

        let last_hash = self.blockchain.last_hash();
        let schema_precommits = schema.precommits(&last_hash);
        let mut precommits = Vec::new();
        for precommit in &schema_precommits {
            precommits.push(PrecommitInfo {
                timestamp: precommit.time(),
                height: precommit.height(),
            });
        }

        let mut chain = HashMap::new();
        for (_, info) in self.shared_api_state.peers_info() {
            chain.insert(info.public_key, info.height);
        }

        HealthCheckInfo {
            height,
            precommits,
            chain,
            service_key,
            consensus_key,
        }
    }
}

impl Api for SystemApi {
    fn wire(&self, router: &mut Router) {
        let _self = self.clone();
        let mempool_info = move |_: &mut Request| -> IronResult<Response> {
            let info = _self.get_mempool_info();
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let _self = self.clone();
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("hash") {
                Some(hash_str) => {
                    let info = _self.get_transaction(hash_str)?;
                    let result = match info {
                        MemPoolResult::Unknown => Self::not_found_response,
                        _ => Self::ok_response,
                    };
                    result(&_self, &::serde_json::to_value(info).unwrap())
                }
                None => {
                    Err(ApiError::IncorrectRequest(
                        "Required parameter of transaction 'hash' is missing".into(),
                    ))?
                }
            }
        };

        let _self = self.clone();
        let healthcheck = move |_: &mut Request| {
            let info = _self.get_healthcheck_info();
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/mempool", mempool_info, "mempool");
        router.get("/v1/transactions/:hash", transaction, "hash");
        router.get("/v1/healthcheck", healthcheck, "healthcheck_info");
    }
}
