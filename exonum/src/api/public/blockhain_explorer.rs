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

use std::num::ParseIntError;
use std::str::ParseBoolError;
use std::cmp;

use params::{Params, Value};
use router::Router;
use iron::prelude::*;

use blockchain::{Block, Blockchain};
use explorer::{BlockInfo, BlockchainExplorer};
use api::{Api, ApiError};
use helpers::Height;

const MAX_BLOCKS_PER_REQUEST: u64 = 1000;

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

    fn get_blocks(
        &self,
        count: u64,
        from: Option<u64>,
        skip_empty_blocks: bool,
    ) -> Result<Vec<Block>, ApiError> {
        if count > MAX_BLOCKS_PER_REQUEST {
            return Err(ApiError::IncorrectRequest(
                format!(
                    "Max block count per request exceeded ({})",
                    MAX_BLOCKS_PER_REQUEST
                ).into(),
            ));
        }
        let explorer = BlockchainExplorer::new(&self.blockchain);
        Ok(explorer.blocks_range(count, from, skip_empty_blocks))
    }

    fn get_block(&self, height: Height) -> Result<Option<BlockInfo>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain);
        Ok(explorer.block_info(height))
    }

    fn get_blocks_with_proofs(&self, from: Height, count: u64) -> Result<Vec<BlockInfo>, ApiError> {
        if count > MAX_BLOCKS_PER_REQUEST {
            return Err(ApiError::IncorrectRequest(
                format!(
                    "Max block count per request exceeded ({})",
                    MAX_BLOCKS_PER_REQUEST
                ).into(),
            ));
        }
        let current_height = self.blockchain.last_block().height().0 + 1;
        let from = from.0;
        let count = cmp::min(current_height.saturating_sub(from), count);
        let to = from + count;

        let explorer = BlockchainExplorer::new(&self.blockchain);
        let block_proofs = {
            let mut block_proofs = Vec::new();
            for height in from..to {
                block_proofs.push(explorer.block_info(Height(height)).unwrap());
            }
            block_proofs
        };
        Ok(block_proofs)
    }
}

impl Api for ExplorerApi {
    fn wire(&self, router: &mut Router) {
        let _self = self.clone();
        let blocks = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            let count: u64 = match map.find(&["count"]) {
                Some(&Value::String(ref count_str)) => {
                    count_str.parse().map_err(|e: ParseIntError| {
                        ApiError::IncorrectRequest(Box::new(e))
                    })?
                }
                _ => {
                    return Err(ApiError::IncorrectRequest(
                        "Required parameter of blocks 'count' is missing".into(),
                    ))?;
                }
            };
            let latest: Option<u64> = match map.find(&["latest"]) {
                Some(&Value::String(ref from_str)) => Some(from_str.parse().map_err(
                    |e: ParseIntError| {
                        ApiError::IncorrectRequest(Box::new(e))
                    },
                )?),
                _ => None,
            };
            let skip_empty_blocks: bool = match map.find(&["skip_empty_blocks"]) {
                Some(&Value::String(ref skip_str)) => {
                    skip_str.parse().map_err(|e: ParseBoolError| {
                        ApiError::IncorrectRequest(Box::new(e))
                    })?
                }
                _ => false,
            };
            let info = _self.get_blocks(count, latest, skip_empty_blocks)?;
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let _self = self.clone();
        let block = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("height") {
                Some(height_str) => {
                    let height: u64 = height_str.parse().map_err(|e: ParseIntError| {
                        ApiError::IncorrectRequest(Box::new(e))
                    })?;
                    let info = _self.get_block(Height(height))?;
                    _self.ok_response(&::serde_json::to_value(info).unwrap())
                }
                None => {
                    Err(ApiError::IncorrectRequest(
                        "Required parameter of block 'height' is missing".into(),
                    ))?
                }
            }
        };

        let _self = self.clone();
        let blocks_with_proofs = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            let count: u64 = match map.find(&["count"]) {
                Some(&Value::String(ref count_str)) => {
                    count_str.parse().map_err(|e: ParseIntError| {
                        ApiError::IncorrectRequest(Box::new(e))
                    })?
                }
                _ => {
                    return Err(ApiError::IncorrectRequest(
                        "Required parameter of blocks 'count' is missing".into(),
                    ))?;
                }
            };
            let from: u64 = match map.find(&["from"]) {
                Some(&Value::String(ref from_str)) => {
                    from_str.parse().map_err(|e: ParseIntError| {
                        ApiError::IncorrectRequest(Box::new(e))
                    })?
                }
                _ => {
                    return Err(ApiError::IncorrectRequest(
                        "Required parameter of blocks 'from' is missing".into(),
                    ))?;
                }
            };
            let info = _self.get_blocks_with_proofs(Height(from), count)?;
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/blocks", blocks, "blocks");
        router.get("/v1/blocks/:height", block, "height");
        router.get(
            "/v1/blocks-with-proofs",
            blocks_with_proofs,
            "blocks_with_proofs",
        );
    }
}
