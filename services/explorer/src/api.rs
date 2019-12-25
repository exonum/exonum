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

//! HTTP API for the explorer service.

pub use exonum_explorer::{api::*, TransactionInfo};

use actix::Arbiter;
use actix_web::{http, ws, AsyncResponder, Error as ActixError, FromRequest, Query};
use exonum::{
    api::{
        backends::actix::{
            self as actix_backend, FutureResponse, HttpRequest, RawHandler, RequestHandler,
        },
        ApiBackend, Error as ApiError, FutureResult,
    },
    blockchain::{Blockchain, CallInBlock, Schema},
    helpers::Height,
    merkledb::{ObjectHash, Snapshot},
    messages::SignedMessage,
    node::{ApiSender, ExternalMessage},
    runtime::{rust::api::ServiceApiScope, ExecutionStatus},
};
use exonum_explorer::{median_precommits_time, BlockchainExplorer};
use futures::{Future, IntoFuture, Sink};
use hex::FromHex;
use serde_json::json;

use std::{
    ops::Bound,
    sync::{Arc, Mutex},
};

use crate::websocket::{Server, Session, SubscriptionType, TransactionFilter};

/// Exonum blockchain explorer API.
#[derive(Debug, Clone)]
pub struct ExplorerApi {
    blockchain: Blockchain,
}

impl ExplorerApi {
    /// Creates a new `ExplorerApi` instance.
    pub fn new(blockchain: Blockchain) -> Self {
        Self { blockchain }
    }

    /// Returns the explored range and the corresponding headers. The range specifies the smallest
    /// and largest heights traversed to collect the number of blocks specified in
    /// the [`BlocksQuery`] struct.
    ///
    /// [`BlocksQuery`]: struct.BlocksQuery.html
    pub fn blocks(
        schema: Schema<&dyn Snapshot>,
        query: BlocksQuery,
    ) -> Result<BlocksRange, ApiError> {
        let explorer = BlockchainExplorer::from_schema(schema);
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
            blocks.last().map_or(Height(0), |info| info.block.height)
        };

        Ok(BlocksRange {
            range: height..upper.next(),
            blocks,
        })
    }

    /// Returns the content for a block at a specific height.
    pub fn block(schema: Schema<&dyn Snapshot>, query: BlockQuery) -> Result<BlockInfo, ApiError> {
        let explorer = BlockchainExplorer::from_schema(schema);
        explorer.block(query.height).map(From::from).ok_or_else(|| {
            ApiError::NotFound(format!(
                "Requested block height ({}) exceeds the blockchain height ({})",
                query.height,
                explorer.height()
            ))
        })
    }

    /// Searches for a transaction, either committed or uncommitted, by the hash.
    pub fn transaction_info(
        schema: Schema<&dyn Snapshot>,
        query: TransactionQuery,
    ) -> Result<TransactionInfo, ApiError> {
        BlockchainExplorer::from_schema(schema)
            .transaction(&query.hash)
            .ok_or_else(|| {
                let description = serde_json::to_string(&json!({ "type": "unknown" })).unwrap();
                ApiError::NotFound(description)
            })
    }

    /// Returns call status of committed transaction.
    pub fn transaction_status(
        schema: Schema<&dyn Snapshot>,
        query: TransactionQuery,
    ) -> Result<CallStatusResponse, ApiError> {
        let explorer = BlockchainExplorer::from_schema(schema);

        let tx_info = explorer.transaction(&query.hash).ok_or_else(|| {
            ApiError::NotFound(format!("Unknown transaction hash ({})", query.hash))
        })?;

        let tx_info = match tx_info {
            TransactionInfo::Committed(info) => info,
            TransactionInfo::InPool { .. } => {
                let err = ApiError::NotFound(format!(
                    "Requested transaction ({}) is not executed yet",
                    query.hash
                ));
                return Err(err);
            }
        };

        let call_in_block = CallInBlock::transaction(tx_info.location().position_in_block());
        let block_height = tx_info.location().block_height();

        let status = ExecutionStatus(explorer.call_status(block_height, call_in_block));
        Ok(CallStatusResponse { status })
    }

    /// Returns call status of `before_transactions` hook.
    pub fn before_transactions_status(
        schema: Schema<&dyn Snapshot>,
        query: CallStatusQuery,
    ) -> Result<CallStatusResponse, ApiError> {
        let explorer = BlockchainExplorer::from_schema(schema);
        let call_in_block = CallInBlock::before_transactions(query.service_id);
        let status = ExecutionStatus(explorer.call_status(query.height, call_in_block));
        Ok(CallStatusResponse { status })
    }

    /// Returns call status of `after_transactions` hook.
    pub fn after_transactions_status(
        schema: Schema<&dyn Snapshot>,
        query: CallStatusQuery,
    ) -> Result<CallStatusResponse, ApiError> {
        let explorer = BlockchainExplorer::from_schema(schema);
        let call_in_block = CallInBlock::after_transactions(query.service_id);
        let status = ExecutionStatus(explorer.call_status(query.height, call_in_block));
        Ok(CallStatusResponse { status })
    }

    /// Adds transaction into the pool of unconfirmed transactions if it's valid
    /// and returns an error otherwise.
    pub fn add_transaction(
        snapshot: &dyn Snapshot,
        sender: &ApiSender,
        query: TransactionHex,
    ) -> FutureResult<TransactionResponse> {
        let verify_message = |snapshot: &dyn Snapshot, hex: String| -> Result<_, failure::Error> {
            let msg = SignedMessage::from_hex(hex)?;
            let tx_hash = msg.object_hash();
            let verified = msg.into_verified()?;
            Blockchain::check_tx(snapshot, &verified)?;
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
            verify_message(snapshot, query.tx_body)
                .into_future()
                .map_err(|e| ApiError::BadRequest(e.to_string()))
                .and_then(send_transaction),
        )
    }

    /// Subscribes to events.
    pub fn handle_ws<Q>(
        name: &str,
        backend: &mut actix_backend::ApiBuilder,
        blockchain: Blockchain,
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
                //.set_broadcast_server_address(address.to_owned().unwrap());
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

    /// Adds explorer API endpoints to the corresponding scope.
    pub fn wire(self, api_scope: &mut ServiceApiScope) {
        // Default subscription for blocks.
        Self::handle_ws(
            "v1/blocks/subscribe",
            api_scope.web_backend(),
            self.blockchain.clone(),
            |_| Ok(SubscriptionType::Blocks),
        );
        // Default subscription for transactions.
        Self::handle_ws(
            "v1/transactions/subscribe",
            api_scope.web_backend(),
            self.blockchain.clone(),
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
            |_| Ok(SubscriptionType::None),
        );

        api_scope
            .endpoint("v1/blocks", |state, query| {
                Self::blocks(state.data().for_core(), query)
            })
            .endpoint("v1/block", |state, query| {
                Self::block(state.data().for_core(), query)
            })
            .endpoint("v1/call_status/transaction", |state, query| {
                Self::transaction_status(state.data().for_core(), query)
            })
            .endpoint("v1/call_status/after_transactions", |state, query| {
                Self::after_transactions_status(state.data().for_core(), query)
            })
            .endpoint("v1/call_status/before_transactions", |state, query| {
                Self::before_transactions_status(state.data().for_core(), query)
            })
            .endpoint("v1/transactions", |state, query| {
                Self::transaction_info(state.data().for_core(), query)
            });

        let tx_sender = self.blockchain.sender().to_owned();
        api_scope.endpoint_mut("v1/transactions", move |state, query| {
            Self::add_transaction(state.snapshot(), &tx_sender, query)
        });
    }
}
