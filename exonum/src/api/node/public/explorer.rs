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

//! Exonum blockchain explorer API.

use actix::Arbiter;
use actix_web::{http, ws, AsyncResponder, Error as ActixError, FromRequest, Query};
use chrono::{DateTime, Utc};
use exonum_merkledb::{ObjectHash, Snapshot};
use futures::{Future, IntoFuture, Sink};
use hex::FromHex;

use std::{
    ops::{Bound, Range},
    sync::{Arc, Mutex},
};

use crate::{
    api::{
        backends::actix::{
            self as actix_backend, FutureResponse, HttpRequest, RawHandler, RequestHandler,
        },
        node::SharedNodeState,
        websocket::{Server, Session, SubscriptionType, TransactionFilter},
        ApiBackend, ApiScope, Error as ApiError, FutureResult,
    },
    blockchain::{Block, Blockchain},
    crypto::Hash,
    explorer::{self, median_precommits_time, BlockchainExplorer, TransactionInfo},
    helpers::Height,
    messages::{Precommit, SignedMessage, Verified},
    node::{ApiSender, ExternalMessage},
    runtime::CallInfo,
};

/// The maximum number of blocks to return per blocks request, in this way
/// the parameter limits the maximum execution time for such requests.
pub const MAX_BLOCKS_PER_REQUEST: usize = 1000;

/// Information on blocks coupled with the corresponding range in the blockchain.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BlocksRange {
    /// Exclusive range of blocks.
    pub range: Range<Height>,
    /// Blocks in the range.
    pub blocks: Vec<BlockInfo>,
}

/// Information about a transaction included in the block.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TxInfo {
    /// Transaction hash.
    pub tx_hash: Hash,
    /// Information to call.
    pub call_info: CallInfo,
}

/// Information about a block in the blockchain.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockInfo {
    /// Block header as recorded in the blockchain.
    #[serde(flatten)]
    pub block: Block,

    /// Precommits authorizing the block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precommits: Option<Vec<Verified<Precommit>>>,

    /// Info of transactions in the block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txs: Option<Vec<TxInfo>>,

    /// Median time from the block precommits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<DateTime<Utc>>,
}

/// Blocks in range parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct BlocksQuery {
    /// The number of blocks to return. Should not be greater than `MAX_BLOCKS_PER_REQUEST`.
    pub count: usize,
    /// The maximum height of the returned blocks.
    ///
    /// The blocks are returned in reverse order,
    /// starting from the latest and at least up to the `latest - count + 1`.
    /// The default value is the height of the latest block in the blockchain.
    pub latest: Option<Height>,
    /// The minimum height of the returned blocks. The default value is `Height(0)` (the genesis
    /// block).
    ///
    /// Note that `earliest` has the least priority compared to `latest` and `count`;
    /// it can only truncate the list of otherwise returned blocks if some of them have a lesser
    /// height.
    pub earliest: Option<Height>,
    /// If true, then only non-empty blocks are returned. The default value is false.
    #[serde(default)]
    pub skip_empty_blocks: bool,
    /// If true, then the returned `BlocksRange`'s `times` field will contain median time from the
    /// corresponding blocks precommits.
    #[serde(default)]
    pub add_blocks_time: bool,
    /// If true, then the returned `BlocksRange.precommits` will contain precommits for the
    /// corresponding returned blocks.
    #[serde(default)]
    pub add_precommits: bool,
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

/// Raw Transaction in hex representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionHex {
    /// The hex value of the transaction to be broadcasted.
    pub tx_body: String,
}

/// Transaction response.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TransactionResponse {
    /// The hex value of the transaction to be broadcasted.
    pub tx_hash: Hash,
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

impl AsRef<str> for TransactionHex {
    fn as_ref(&self) -> &str {
        self.tx_body.as_ref()
    }
}

impl AsRef<[u8]> for TransactionHex {
    fn as_ref(&self) -> &[u8] {
        self.tx_body.as_ref()
    }
}

/// Exonum blockchain explorer API.
#[derive(Debug, Clone)]
pub struct ExplorerApi {
    blockchain: Blockchain,
}

impl ExplorerApi {
    /// Create a new `ExplorerApi` instance.
    pub fn new(blockchain: Blockchain) -> Self {
        Self { blockchain }
    }

    /// Return the explored range and the corresponding headers. The range specifies the smallest
    /// and largest heights traversed to collect the number of blocks specified in
    /// the [`BlocksQuery`] struct.
    ///
    /// [`BlocksQuery`]: struct.BlocksQuery.html
    pub fn blocks(snapshot: &dyn Snapshot, query: BlocksQuery) -> Result<BlocksRange, ApiError> {
        let explorer = BlockchainExplorer::new(snapshot);
        if query.count > MAX_BLOCKS_PER_REQUEST {
            return Err(ApiError::BadRequest(format!(
                "Max block count per request exceeded ({})",
                MAX_BLOCKS_PER_REQUEST
            )));
        }

        let (upper, upper_bound) = if let Some(upper) = query.latest {
            if upper > explorer.height() {
                return Err(ApiError::NotFound(format!(
                    "Requested latest height {} is greater than the current blockchain height {}",
                    upper,
                    explorer.height()
                )));
            }
            (upper, Bound::Included(upper))
        } else {
            (explorer.height(), Bound::Unbounded)
        };
        let lower_bound = if let Some(lower) = query.earliest {
            Bound::Included(lower)
        } else {
            Bound::Unbounded
        };

        let blocks: Vec<_> = explorer
            .blocks((lower_bound, upper_bound))
            .rev()
            .filter(|block| !query.skip_empty_blocks || !block.is_empty())
            .take(query.count)
            .map(|block| BlockInfo {
                txs: None,

                time: if query.add_blocks_time {
                    Some(median_precommits_time(&block.precommits()))
                } else {
                    None
                },

                precommits: if query.add_precommits {
                    Some(block.precommits().to_vec())
                } else {
                    None
                },

                block: block.into_header(),
            })
            .collect();

        let height = if blocks.len() < query.count {
            query.earliest.unwrap_or(Height(0))
        } else {
            blocks.last().map_or(Height(0), |info| info.block.height())
        };

        Ok(BlocksRange {
            range: height..upper.next(),
            blocks,
        })
    }

    /// Return the content for a block at a specific height.
    pub fn block(snapshot: &dyn Snapshot, query: BlockQuery) -> Result<BlockInfo, ApiError> {
        let explorer = BlockchainExplorer::new(snapshot);
        explorer.block(query.height).map(From::from).ok_or_else(|| {
            ApiError::NotFound(format!(
                "Requested block height ({}) exceeds the blockchain height ({})",
                query.height,
                explorer.height()
            ))
        })
    }

    /// Search for a transaction, either committed or uncommitted, by the hash.
    pub fn transaction_info(
        snapshot: &dyn Snapshot,
        query: TransactionQuery,
    ) -> Result<TransactionInfo, ApiError> {
        BlockchainExplorer::new(snapshot)
            .transaction(&query.hash)
            .ok_or_else(|| {
                let description = serde_json::to_string(&json!({ "type": "unknown" })).unwrap();
                ApiError::NotFound(description)
            })
    }

    /// Add transaction into the pool of unconfirmed transactions, and broadcast transaction to other nodes.
    // TODO move this method to the public system API [ECR-3222]
    pub fn add_transaction(
        sender: &ApiSender,
        query: TransactionHex,
    ) -> FutureResult<TransactionResponse> {
        let verify_message = |hex: String| -> Result<_, failure::Error> {
            let msg = SignedMessage::from_hex(hex)?;
            let tx_hash = msg.object_hash();
            let verified = msg.into_verified()?;
            Ok((verified, tx_hash))
        };

        let sender = sender.clone();
        let send_transaction = move |(verified, tx_hash)| {
            sender
                .clone()
                .0
                .send(ExternalMessage::Transaction(verified))
                .map(move |_| TransactionResponse { tx_hash })
                .map_err(|e| ApiError::InternalError(e.into()))
        };

        Box::new(
            verify_message(query.tx_body)
                .into_future()
                .map_err(|e| ApiError::BadRequest(e.to_string()))
                .and_then(send_transaction),
        )
    }

    /// Subscribes to events.
    pub fn handle_ws<Q>(
        name: &'static str,
        backend: &mut actix_backend::ApiBuilder,
        blockchain: Blockchain,
        shared_node_state: SharedNodeState,
        extract_query: Q,
    ) where
        Q: Fn(&HttpRequest) -> Result<SubscriptionType, ActixError> + Send + Sync + 'static,
    {
        let server = Arc::new(Mutex::new(None));

        let index = move |request: HttpRequest| -> FutureResponse {
            let server = server.clone();
            let blockchain = blockchain.clone();
            let mut address = server.lock().expect("Expected mutex lock");
            if address.is_none() {
                *address = Some(Arbiter::start(|_| Server::new(blockchain)));

                shared_node_state.set_broadcast_server_address(address.to_owned().unwrap());
            }
            let address = address.to_owned().unwrap();

            extract_query(&request)
                .into_future()
                .from_err()
                .and_then(move |query: SubscriptionType| {
                    ws::start(&request, Session::new(address, vec![query])).into_future()
                })
                .responder()
        };

        backend.raw_handler(RequestHandler {
            name: name.to_owned(),
            method: http::Method::GET,
            inner: Arc::from(index) as Arc<RawHandler>,
        });
    }

    /// Add explorer API endpoints to the corresponding scope.
    pub fn wire(
        self,
        api_scope: &mut ApiScope,
        shared_node_state: SharedNodeState,
    ) -> &mut ApiScope {
        // Default subscription for blocks.
        Self::handle_ws(
            "v1/blocks/subscribe",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_node_state.clone(),
            |_| Ok(SubscriptionType::Blocks),
        );
        // Default subscription for transactions.
        Self::handle_ws(
            "v1/transactions/subscribe",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_node_state.clone(),
            |request| {
                if request.query().is_empty() {
                    return Ok(SubscriptionType::Transactions { filter: None });
                }

                Query::from_request(request, &Default::default())
                    .map(|query: Query<TransactionFilter>| {
                        Ok(SubscriptionType::Transactions {
                            filter: Some(query.into_inner()),
                        })
                    })
                    .unwrap_or(Ok(SubscriptionType::None))
            },
        );
        // Default websocket connection.
        Self::handle_ws(
            "v1/ws",
            api_scope.web_backend(),
            self.blockchain.clone(),
            shared_node_state,
            |_| Ok(SubscriptionType::None),
        );
        api_scope
            .endpoint("v1/blocks", {
                let blockchain = self.blockchain.clone();
                move |query| Self::blocks(blockchain.snapshot().as_ref(), query)
            })
            .endpoint("v1/block", {
                let blockchain = self.blockchain.clone();
                move |query| Self::block(blockchain.snapshot().as_ref(), query)
            })
            .endpoint("v1/transactions", {
                let blockchain = self.blockchain.clone();
                move |query| Self::transaction_info(blockchain.snapshot().as_ref(), query)
            })
            .endpoint_mut("v1/transactions", {
                let blockchain = self.blockchain.clone();
                move |query| Self::add_transaction(blockchain.sender(), query)
            })
    }
}

impl<'a> From<explorer::BlockInfo<'a>> for BlockInfo {
    fn from(inner: explorer::BlockInfo<'a>) -> Self {
        Self {
            block: inner.header().clone(),
            precommits: Some(inner.precommits().to_vec()),
            txs: Some(
                inner
                    .transaction_hashes()
                    .iter()
                    .enumerate()
                    .map(|(idx, &tx_hash)| TxInfo {
                        tx_hash,
                        call_info: inner
                            .transaction(idx)
                            .unwrap()
                            .content()
                            .payload()
                            .call_info
                            .clone(),
                    })
                    .collect(),
            ),
            time: Some(median_precommits_time(&inner.precommits())),
        }
    }
}
