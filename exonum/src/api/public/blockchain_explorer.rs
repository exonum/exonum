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

use router::Router;
use iron::prelude::*;

use std::ops::Range;

use blockchain::{Block, Blockchain};
use explorer::{BlockchainExplorer, TransactionInfo};
use api::{Api, ApiError};
use crypto::Hash;
use helpers::Height;

const MAX_BLOCKS_PER_REQUEST: usize = 1000;

/// Information on blocks coupled with the corresponding range in the blockchain.
#[derive(Debug, Serialize, Deserialize)]
pub struct BlocksRange {
    /// Exclusive range of blocks.
    pub range: Range<Height>,
    /// Blocks in the range.
    pub blocks: Vec<Block>,
}

/// Public explorer API.
#[derive(Clone, Debug)]
pub struct ExplorerApi {
    blockchain: Blockchain,
}

impl ExplorerApi {
    /// Creates a new `ExplorerApi` instance.
    pub fn new(blockchain: Blockchain) -> Self {
        ExplorerApi { blockchain }
    }

    fn explorer(&self) -> BlockchainExplorer {
        BlockchainExplorer::new(&self.blockchain)
    }

    fn blocks(
        &self,
        count: usize,
        upper: Option<Height>,
        skip_empty_blocks: bool,
    ) -> Result<BlocksRange, ApiError> {
        if count > MAX_BLOCKS_PER_REQUEST {
            return Err(ApiError::BadRequest(format!(
                "Max block count per request exceeded ({})",
                MAX_BLOCKS_PER_REQUEST
            )));
        }

        let explorer = self.explorer();
        let (upper, blocks_iter) = if let Some(upper) = upper {
            (upper, explorer.blocks(..upper.next()))
        } else {
            (explorer.height(), explorer.blocks(..))
        };

        let blocks: Vec<_> = blocks_iter
            .rev()
            .filter(|block| !skip_empty_blocks || !block.is_empty())
            .take(count)
            .map(|block| block.into_header())
            .collect();

        let height = if blocks.len() < count {
            Height(0)
        } else {
            blocks.last().map_or(Height(0), |block| block.height())
        };

        Ok(BlocksRange {
            range: height..upper.next(),
            blocks,
        })
    }

    fn transaction_info(&self, hash: &Hash) -> Option<TransactionInfo> {
        self.explorer().transaction(hash)
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
            let explorer = self.explorer();
            self.ok_response(&::serde_json::to_value(explorer.block(height)).unwrap())
        };

        router.get("/v1/blocks/:height", block, "height");
    }

    fn set_transaction_info_response(self, router: &mut Router) {
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            let hash: Hash = self.url_fragment(req, "hash")?;

            match self.transaction_info(&hash) {
                None => self.not_found_response(&json!({ "type": "unknown" })),
                Some(info) => self.ok_response(&::serde_json::to_value(info).unwrap()),
            }
        };
        let post_transaction = move |req: &mut Request| -> IronResult<Response> {
           // let PeerAddInfo { ip } = self.parse_body(request)?;
            //self.node_channel.peer_add(ip).map_err(ApiError::from)?;
            unimplemented!();
        };

        router.get("/v1/transaction/:hash", transaction, "hash");
        router.post("/v1/transaction/", post_transaction, "transaction");
    }
}

impl Api for ExplorerApi {
    fn wire(&self, router: &mut Router) {
        self.clone().set_blocks_response(router);
        self.clone().set_block_response(router);
        self.clone().set_transaction_info_response(router);
    }
}
