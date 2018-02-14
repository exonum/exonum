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
            return Err(ApiError::BadRequest(format!(
                "Max block count per request exceeded ({})",
                MAX_BLOCKS_PER_REQUEST
            )));
        }
        let explorer = BlockchainExplorer::new(&self.blockchain);
        Ok(explorer.blocks_range(count, from, skip_empty_blocks))
    }

    fn get_block(&self, height: Height) -> Result<Option<BlockInfo>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain);
        Ok(explorer.block_info(height))
    }
}

impl Api for ExplorerApi {
    fn wire(&self, router: &mut Router) {

        let self_ = self.clone();
        let blocks = move |req: &mut Request| -> IronResult<Response> {
            let count: u64 = self_.required_param(req, "count")?;
            let latest: Option<u64> = self_.optional_param(req, "latest")?;
            let skip_empty_blocks: bool = self_
                .optional_param(req, "skip_empty_blocks")?
                .unwrap_or(false);
            let info = self_.get_blocks(count, latest, skip_empty_blocks)?;
            self_.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let block = move |req: &mut Request| -> IronResult<Response> {
            let height: u64 = self_.url_fragment(req, "height")?;
            let info = self_.get_block(Height(height))?;
            self_.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/blocks", blocks, "blocks");
        router.get("/v1/blocks/:height", block, "height");
    }
}
