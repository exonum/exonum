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

use router::Router;
use serde_json::Value as JsonValue;
use iron::prelude::*;

use blockchain::Blockchain;
use explorer::{BlockchainExplorer, BlocksRange, TransactionInfo as TxInfo};
use node::state::TxPool;
use api::{Api, ApiError};
use crypto::Hash;
use helpers::Height;

const MAX_BLOCKS_PER_REQUEST: usize = 1000;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum TransactionInfo {
    Unknown,
    InPool { content: JsonValue },
    Committed(TxInfo),
}

/// Public explorer API.
#[derive(Clone, Debug)]
pub struct ExplorerApi {
    explorer: BlockchainExplorer,
    pool: TxPool,
}

impl ExplorerApi {
    /// Creates a new `ExplorerApi` instance.
    pub fn new(pool: TxPool, blockchain: Blockchain) -> Self {
        ExplorerApi {
            pool,
            explorer: BlockchainExplorer::new(blockchain),
        }
    }

    fn explorer(&self) -> &BlockchainExplorer {
        &self.explorer
    }

    fn blocks(
        &self,
        count: usize,
        from: Option<Height>,
        skip_empty_blocks: bool,
    ) -> Result<BlocksRange, ApiError> {
        if count > MAX_BLOCKS_PER_REQUEST {
            return Err(ApiError::BadRequest(format!(
                "Max block count per request exceeded ({})",
                MAX_BLOCKS_PER_REQUEST
            )));
        }
        Ok(self.explorer().blocks_range(count, from, skip_empty_blocks))
    }

    fn transaction_info(&self, hash: &Hash) -> Result<TransactionInfo, ApiError> {
        if let Some(tx) = self.pool.read().expect("Uanble to read pool").get(hash) {
            Ok(TransactionInfo::InPool {
                content: tx.serialize_field().map_err(ApiError::InternalError)?,
            })
        } else if let Some(tx_info) = self.explorer().transaction(hash) {
            Ok(TransactionInfo::Committed(tx_info))
        } else {
            Ok(TransactionInfo::Unknown)
        }
    }

    fn set_blocks_response(self, router: &mut Router) {
        let blocks = move |req: &mut Request| -> IronResult<Response> {
            let count: usize = self.required_param(req, "count")?;
            let latest: Option<u64> = self.optional_param(req, "latest")?;
            let skip_empty_blocks: bool = self.optional_param(req, "skip_empty_blocks")?
                .unwrap_or(false);
            let info = self.blocks(count, latest.map(Height), skip_empty_blocks)?;
            self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/blocks", blocks, "blocks");
    }

    fn set_block_response(self, router: &mut Router) {
        let block = move |req: &mut Request| -> IronResult<Response> {
            let height: Height = self.url_fragment(req, "height")?;
            let info = self.explorer().block(height);
            self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/blocks/:height", block, "height");
    }

    fn set_transaction_info_response(self, router: &mut Router) {
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            let hash: Hash = self.url_fragment(req, "hash")?;
            let info = self.transaction_info(&hash)?;
            let result = match info {
                TransactionInfo::Unknown => Self::not_found_response,
                _ => Self::ok_response,
            };
            result(&self, &::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/transactions/:hash", transaction, "hash");
    }
}

impl Api for ExplorerApi {
    fn wire(&self, router: &mut Router) {
        self.clone().set_blocks_response(router);
        self.clone().set_block_response(router);
        self.clone().set_transaction_info_response(router);
    }
}
