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
use iron::prelude::*;

use blockchain::{Blockchain, Block};
use explorer::{BlockInfo, BlockchainExplorer, TxInfo};
use node::state::TxPool;
use api::{Api, ApiError};
use crypto::Hash;
use helpers::Height;

const MAX_BLOCKS_PER_REQUEST: u64 = 1000;

#[derive(Serialize)]
#[serde(tag = "type")]
enum TransactionInfo {
    Unknown,
    InPool(PoolTransactionInfo),
    Committed(CommittedTransactionInfo),
}

#[derive(Serialize)]
struct PoolTransactionInfo {
    content: ::serde_json::Value,
}

#[derive(Serialize)]
struct CommittedTransactionInfo {
    tx_info: TxInfo,
}

/// Public explorer API.
#[derive(Clone, Debug)]
pub struct ExplorerApi {
    blockchain: Blockchain,
    pool: TxPool,
}

impl ExplorerApi {
    /// Creates a new `ExplorerApi` instance.
    pub fn new(pool: TxPool, blockchain: Blockchain) -> Self {
        ExplorerApi { pool, blockchain }
    }

    fn blocks(
        &self,
        count: u64,
        from: Option<u64>,
        skip_empty_blocks: bool,
    ) -> Result<Vec<Block>, ApiError> {
        if count > MAX_BLOCKS_PER_REQUEST {
            return Err(ApiError::BadRequest(format!(
                "Max block count per request exceeded ({})",
                MAX_BLOCKS_PER_REQUEST
            )));
        }
        let explorer = BlockchainExplorer::new(&self.blockchain);
        Ok(explorer.blocks_range(count, from, skip_empty_blocks))
    }

    fn block(&self, height: Height) -> Option<BlockInfo> {
        let explorer = BlockchainExplorer::new(&self.blockchain);
        explorer.block_info(height)
    }

    fn transaction_info(&self, hash: &Hash) -> Result<TransactionInfo, ApiError> {
        if let Some(tx) = self.pool.read().expect("Uanble to read pool").get(hash) {
            Ok(TransactionInfo::InPool(PoolTransactionInfo {
                content: tx.serialize_field().map_err(ApiError::InternalError)?,
            }))
        } else if let Some(tx_info) = BlockchainExplorer::new(&self.blockchain).tx_info(hash)? {
            Ok(TransactionInfo::Committed(CommittedTransactionInfo{tx_info}))
        } else {
            Ok(TransactionInfo::Unknown)
        }
    }
}

impl Api for ExplorerApi {
    fn wire(&self, router: &mut Router) {
        set_blocks_response(self.clone(), router);
        set_block_response(self.clone(), router);
        set_transaction_info_response(self.clone(), router);
    }
}

fn set_blocks_response(api: ExplorerApi, router: &mut Router) {
    let blocks = move |req: &mut Request| -> IronResult<Response> {
        let count: u64 = api.required_param(req, "count")?;
        let latest: Option<u64> = api.optional_param(req, "latest")?;
        let skip_empty_blocks: bool = api.optional_param(req, "skip_empty_blocks")?.unwrap_or(
            false,
        );
        let info = api.blocks(count, latest, skip_empty_blocks)?;
        api.ok_response(&::serde_json::to_value(info).unwrap())
    };

    router.get("/v1/blocks", blocks, "blocks");
}

fn set_block_response(api: ExplorerApi, router: &mut Router) {
    let block = move |req: &mut Request| -> IronResult<Response> {
        let height: u64 = api.url_fragment(req, "height")?;
        let info = api.block(Height(height));
        api.ok_response(&::serde_json::to_value(info).unwrap())
    };

    router.get("/v1/blocks/:height", block, "height");
}

fn set_transaction_info_response(api: ExplorerApi, router: &mut Router) {
    let transaction = move |req: &mut Request| -> IronResult<Response> {
        let hash: Hash = api.url_fragment(req, "hash")?;
        let info = api.transaction_info(&hash)?;
        let result = match info {
            TransactionInfo::Unknown => ExplorerApi::not_found_response,
            _ => ExplorerApi::ok_response,
        };
        result(&api, &::serde_json::to_value(info).unwrap())
    };

    router.get("/v1/transactions/:hash", transaction, "hash");
}
