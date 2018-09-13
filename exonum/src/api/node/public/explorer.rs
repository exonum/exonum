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

//! Exonum blockchain explorer API.

use actix::Arbiter;
use actix_web::{http, ws};
use futures::IntoFuture;
use serde_json;

use std::ops::Range;
use std::sync::{Arc, Mutex};

use api::{
    backends::actix::{self, FutureResponse, HttpRequest, RawHandler, RequestHandler},
    websocket::{Server, Session}, Error as ApiError, ServiceApiBackend, ServiceApiScope,
    ServiceApiState,
};
use blockchain::{Block, SharedNodeState};
use crypto::Hash;
use explorer::{BlockchainExplorer, TransactionInfo};
use helpers::Height;
use messages::Precommit;

/// The maximum number of blocks to return per blocks request, in this way
/// the parameter limits the maximum execution time for such requests.
pub const MAX_BLOCKS_PER_REQUEST: usize = 1000;

/// Information on blocks coupled with the corresponding range in the blockchain.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BlocksRange {
    /// Exclusive range of blocks.
    pub range: Range<Height>,
    /// Blocks in the range.
    pub blocks: Vec<Block>,
}

/// Information about a block in the blockchain.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockInfo {
    /// Block header as recorded in the blockchain.
    pub block: Block,
    /// Precommits authorizing the block.
    pub precommits: Vec<Precommit>,
    /// Hashes of transactions in the block.
    pub txs: Vec<Hash>,
}

/// Blocks in range parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct BlocksQuery {
    /// The number of blocks to return. Should not be greater than `MAX_BLOCKS_PER_REQUEST`.
    pub count: usize,
    /// The maximum height of the returned blocks. The blocks are returned in reverse order,
    /// starting from the latest and at least up to the `latest` - `count` + 1.
    /// The default value is the height of the latest block in the blockchain.
    pub latest: Option<Height>,
    /// If true, then only non-empty blocks are returned. The default value is false.
    #[serde(default)]
    pub skip_empty_blocks: bool,
}

/// Block query parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BlockQuery {
    /// The height of the desired block.
    pub height: Height,
}

impl BlockQuery {
    /// Creates a new block query with the given height.
    pub fn new(height: Height) -> Self {
        Self { height }
    }
}

/// Transaction query parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TransactionQuery {
    /// The hash of the transaction to be searched.
    pub hash: Hash,
}

impl TransactionQuery {
    /// Creates a new transaction query with the given height.
    pub fn new(hash: Hash) -> Self {
        Self { hash }
    }
}

/// Exonum blockchain explorer API.
#[derive(Debug, Clone, Copy)]
pub struct ExplorerApi;

impl ExplorerApi {
    /// Returns the explored range and the corresponding headers. The range specifies the smallest
    /// and largest heights traversed to collect the number of blocks specified in
    /// the [`BlocksQuery`] struct.
    ///
    /// [`BlocksQuery`]: struct.BlocksQuery.html
    pub fn blocks(state: &ServiceApiState, query: BlocksQuery) -> Result<BlocksRange, ApiError> {
        let explorer = BlockchainExplorer::new(state.blockchain());
        if query.count > MAX_BLOCKS_PER_REQUEST {
            return Err(ApiError::BadRequest(format!(
                "Max block count per request exceeded ({})",
                MAX_BLOCKS_PER_REQUEST
            )));
        }

        let (upper, blocks_iter) = if let Some(upper) = query.latest {
            (upper, explorer.blocks(..upper.next()))
        } else {
            (explorer.height(), explorer.blocks(..))
        };

        let blocks: Vec<_> = blocks_iter
            .rev()
            .filter(|block| !query.skip_empty_blocks || !block.is_empty())
            .take(query.count)
            .map(|block| block.into_header())
            .collect();

        let height = if blocks.len() < query.count {
            Height(0)
        } else {
            blocks.last().map_or(Height(0), |block| block.height())
        };

        Ok(BlocksRange {
            range: height..upper.next(),
            blocks,
        })
    }

    /// Returns the content for a block at a specific height.
    pub fn block(
        state: &ServiceApiState,
        query: BlockQuery,
    ) -> Result<Option<BlockInfo>, ApiError> {
        Ok(BlockchainExplorer::new(state.blockchain())
            .block(query.height)
            .map(From::from))
    }

    /// Searches for a transaction, either committed or uncommitted, by the hash.
    pub fn transaction_info(
        state: &ServiceApiState,
        query: TransactionQuery,
    ) -> Result<TransactionInfo, ApiError> {
        BlockchainExplorer::new(state.blockchain())
            .transaction(&query.hash)
            .ok_or_else(|| {
                let description = serde_json::to_string(&json!({ "type": "unknown" })).unwrap();
                debug!("{}", description);
                ApiError::NotFound(description)
            })
    }

    /// Subscribes to block commits events.
    pub fn handle_subscribe(
        name: &'static str,
        backend: &mut actix::ApiBuilder,
        service_api_state: ServiceApiState,
        shared_node_state: SharedNodeState,
    ) {
        let server = Arc::new(Mutex::new(None));
        let service_api_state = Arc::new(service_api_state);

        let index = move |req: HttpRequest| -> FutureResponse {
            let server = server.clone();
            let service_api_state = service_api_state.clone();
            let mut address = server.lock().expect("Expected mutex lock");
            if address.is_none() {
                *address = Some(Arbiter::start(|_| Server::new(service_api_state)));

                shared_node_state.set_broadcast_server_address(address.to_owned().unwrap());
            }

            Box::new(ws::start(&req, Session::new(address.to_owned().unwrap())).into_future())
        };

        backend.raw_handler(RequestHandler {
            name: name.to_owned(),
            method: http::Method::GET,
            inner: Arc::from(index) as Arc<RawHandler>,
        });
    }

    /// Adds explorer API endpoints to the corresponding scope.
    pub fn wire(
        api_scope: &mut ServiceApiScope,
        service_api_state: ServiceApiState,
        shared_node_state: SharedNodeState,
    ) -> &mut ServiceApiScope {
        Self::handle_subscribe(
            "v1/blocks/subscribe",
            api_scope.web_backend(),
            service_api_state,
            shared_node_state,
        );
        api_scope
            .endpoint("v1/blocks", Self::blocks)
            .endpoint("v1/block", Self::block)
            .endpoint("v1/transactions", Self::transaction_info)
    }
}

impl<'a> From<::explorer::BlockInfo<'a>> for BlockInfo {
    fn from(inner: ::explorer::BlockInfo<'a>) -> Self {
        Self {
            block: inner.header().clone(),
            precommits: inner.precommits().to_vec(),
            txs: inner.transaction_hashes().to_vec(),
        }
    }
}
