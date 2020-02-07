// Copyright 2020 The Exonum Team
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

//! HTTP API for the explorer service. All APIs are accessible from the public HTTP server
//! of the node.
//!
//! # Table of Contents
//!
//! - [List blocks](#list-blocks)
//! - [Get specific block](#get-specific-block)
//! - [Get transaction by hash](#transaction-by-hash)
//! - Call status:
//!
//!     - [for transactions](#call-status-for-transaction)
//!     - [for `before_transactions` hook](#call-status-for-before_transactions-hook)
//!     - [for `after_transactions` hook](#call-status-for-after_transactions-hook)
//!
//! - [Submit transaction](#submit-transaction)
//!
//! # List Blocks
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/explorer/v1/blocks` |
//! | Method      | GET   |
//! | Query type  | [`BlockQuery`] |
//! | Return type | [`BlockInfo`] |
//!
//! Returns the explored range and the corresponding headers. The range specifies the smallest
//! and largest heights traversed to collect the blocks.
//!
//! [`BlocksQuery`]: struct.BlocksQuery.html
//! [`BlocksRange`]: struct.BlocksRange.html
//!
//! ```
//! # use exonum::helpers::Height;
//! # use exonum_explorer_service::{api::BlocksRange, ExplorerFactory};
//! # use exonum_testkit::TestKitBuilder;
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_default_rust_service(ExplorerFactory)
//!     .build();
//! testkit.create_blocks_until(Height(5));
//!
//! let api = testkit.api();
//! let response: BlocksRange = reqwest::Client::new()
//!     .get(&api.public_url("api/explorer/v1/blocks?count=2"))
//!     .send()?
//!     .error_for_status()?
//!     .json()?;
//! assert_eq!(response.range, Height(4)..Height(6));
//! // Blocks are returned in reverse order, from the latest
//! // to the earliest.
//! assert_eq!(response.blocks[0].block.height, Height(5));
//! assert_eq!(response.blocks[1].block.height, Height(4));
//! # Ok(())
//! # }
//! ```
//!
//! # Get Specific Block
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/explorer/v1/block` |
//! | Method      | GET   |
//! | Query type  | [`BlockQuery`] |
//! | Return type | [`BlockInfo`] |
//!
//! Returns the content for a block at a specific `height`.
//!
//! [`BlockQuery`]: struct.BlockQuery.html
//! [`BlockInfo`]: struct.BlockInfo.html
//!
//! ```
//! # use exonum::helpers::Height;
//! # use exonum_explorer_service::{api::BlockInfo, ExplorerFactory};
//! # use exonum_testkit::TestKitBuilder;
//! # fn main() -> Result<(), failure::Error> {
//! # let mut testkit = TestKitBuilder::validator()
//! #    .with_default_rust_service(ExplorerFactory)
//! #    .build();
//! testkit.create_blocks_until(Height(5));
//!
//! let api = testkit.api();
//! let response: BlockInfo = reqwest::Client::new()
//!     .get(&api.public_url("api/explorer/v1/block?height=3"))
//!     .send()?
//!     .error_for_status()?
//!     .json()?;
//! assert_eq!(response.block.height, Height(3));
//! // Precommits and median precommit time are always returned.
//! assert!(response.precommits.is_some());
//! assert!(response.time.is_some());
//! # Ok(())
//! # }
//! ```
//!
//! # Transaction by Hash
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/explorer/v1/transactions` |
//! | Method      | GET   |
//! | Query type  | [`TransactionQuery`] |
//! | Return type | [`TransactionInfo`] |
//!
//! Searches for a transaction, either committed or uncommitted, by the hash.
//!
//! [`TransactionQuery`]: struct.TransactionQuery.html
//! [`TransactionInfo`]: enum.TransactionInfo.html
//!
//! ```
//! # use exonum::{
//! #     crypto::gen_keypair, helpers::Height, merkledb::ObjectHash, runtime::ExecutionError,
//! # };
//! # use exonum_rust_runtime::{ExecutionContext, DefaultInstance, Service, ServiceFactory};
//! # use exonum_derive::*;
//! # use exonum_explorer_service::{api::{TransactionQuery, TransactionInfo}, ExplorerFactory};
//! # use exonum_testkit::TestKitBuilder;
//! #[exonum_interface]
//! trait ServiceInterface<Ctx> {
//!     type Output;
//!     #[interface_method(id = 0)]
//!     fn do_nothing(&self, ctx: Ctx, _seed: u32) -> Self::Output;
//! }
//!
//! #[derive(Debug, ServiceDispatcher, ServiceFactory)]
//! # #[service_factory(artifact_name = "my-service")]
//! #[service_dispatcher(implements("ServiceInterface"))]
//! struct MyService;
//! // Some implementations skipped for `MyService`...
//! # impl ServiceInterface<ExecutionContext<'_>> for MyService {
//! #    type Output = Result<(), ExecutionError>;
//! #    fn do_nothing(&self, ctx: ExecutionContext<'_>, _seed: u32) -> Self::Output { Ok(()) }
//! # }
//! # impl DefaultInstance for MyService {
//! #     const INSTANCE_ID: u32 = 100;
//! #     const INSTANCE_NAME: &'static str = "my-service";
//! # }
//! # impl Service for MyService {}
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!    .with_default_rust_service(ExplorerFactory)
//!    .with_default_rust_service(MyService)
//!    .build();
//! let tx = gen_keypair().do_nothing(MyService::INSTANCE_ID, 0);
//! testkit.create_block_with_transaction(tx.clone());
//!
//! let api = testkit.api();
//! let response: TransactionInfo = reqwest::Client::new()
//!     .get(&api.public_url("api/explorer/v1/transactions"))
//!     .query(&TransactionQuery { hash: tx.object_hash() })
//!     .send()?
//!     .error_for_status()?
//!     .json()?;
//! let response = response.as_committed().unwrap();
//! assert_eq!(response.location().block_height(), Height(1));
//! # Ok(())
//! # }
//! ```
//!
//! # Call Status for Transaction
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/explorer/v1/call_status/transaction` |
//! | Method      | GET   |
//! | Query type  | [`TransactionQuery`] |
//! | Return type | [`CallStatusResponse`] |
//!
//! Returns call status of committed transaction.
//!
//! [`CallStatusResponse`]: struct.CallStatusResponse.html
//!
//! ```
//! # use exonum::{
//! #     crypto::gen_keypair, helpers::Height, merkledb::ObjectHash,
//! #     runtime::{ExecutionError, ExecutionFail},
//! # };
//! # use exonum_rust_runtime::{ExecutionContext, DefaultInstance, Service, ServiceFactory};
//! # use exonum_derive::*;
//! # use exonum_explorer_service::{api::{TransactionQuery, CallStatusResponse}, ExplorerFactory};
//! # use exonum_testkit::TestKitBuilder;
//! #[exonum_interface]
//! trait ServiceInterface<Ctx> {
//!     type Output;
//!     #[interface_method(id = 0)]
//!     fn cause_error(&self, ctx: Ctx, _seed: u32) -> Self::Output;
//! }
//!
//! #[derive(Debug, ServiceDispatcher, ServiceFactory)]
//! # #[service_factory(artifact_name = "my-service")]
//! #[service_dispatcher(implements("ServiceInterface"))]
//! struct MyService;
//! // Some implementations skipped for `MyService`...
//! # impl ServiceInterface<ExecutionContext<'_>> for MyService {
//! #    type Output = Result<(), ExecutionError>;
//! #    fn cause_error(&self, ctx: ExecutionContext<'_>, _seed: u32) -> Self::Output {
//! #        Err(ExecutionError::service(0, "Error!"))
//! #    }
//! # }
//! # impl DefaultInstance for MyService {
//! #     const INSTANCE_ID: u32 = 100;
//! #     const INSTANCE_NAME: &'static str = "my-service";
//! # }
//! # impl Service for MyService {}
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!    .with_default_rust_service(MyService)
//!    .with_default_rust_service(ExplorerFactory)
//!    .build();
//! let tx = gen_keypair().cause_error(MyService::INSTANCE_ID, 0);
//! testkit.create_block_with_transaction(tx.clone());
//!
//! let api = testkit.api();
//! let response: CallStatusResponse = reqwest::Client::new()
//!     .get(&api.public_url("api/explorer/v1/call_status/transaction"))
//!     .query(&TransactionQuery { hash: tx.object_hash() })
//!     .send()?
//!     .error_for_status()?
//!     .json()?;
//! let err = response.status.0.unwrap_err();
//! assert_eq!(err.description(), "Error!");
//! # Ok(())
//! # }
//! ```
//!
//! # Call Status for `before_transactions` hook
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/explorer/v1/call_status/before_transactions` |
//! | Method      | GET   |
//! | Query type  | [`CallStatusQuery`] |
//! | Return type | [`CallStatusResponse`] |
//!
//! Returns call status of a `before_transactions` hook for a specific service at a specific height.
//! Note that the endpoint returns the normal execution status `Ok(())` if the queried service
//! was not active at the specified height.
//!
//! [`CallStatusQuery`]: struct.CallStatusQuery.html
//!
//! ```
//! # use exonum::{
//! #     crypto::gen_keypair, helpers::Height, merkledb::ObjectHash,
//! #     runtime::{ExecutionError, ExecutionFail},
//! # };
//! # use exonum_rust_runtime::{ExecutionContext, DefaultInstance, Service, ServiceFactory};
//! # use exonum_derive::*;
//! # use exonum_explorer_service::{api::{CallStatusQuery, CallStatusResponse}, ExplorerFactory};
//! # use exonum_testkit::TestKitBuilder;
//! #[derive(Debug, ServiceDispatcher, ServiceFactory)]
//! # #[service_factory(artifact_name = "my-service")]
//! struct MyService;
//! // Some implementations skipped for `MyService`...
//! # impl DefaultInstance for MyService {
//! #     const INSTANCE_ID: u32 = 100;
//! #     const INSTANCE_NAME: &'static str = "my-service";
//! # }
//! # impl Service for MyService {
//! #     fn before_transactions(&self, ctx: ExecutionContext<'_>) -> Result<(), ExecutionError> {
//! #         Err(ExecutionError::service(0, "Not a good start"))
//! #     }
//! # }
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!    .with_default_rust_service(MyService)
//!    .with_default_rust_service(ExplorerFactory)
//!    .build();
//! testkit.create_blocks_until(Height(5));
//!
//! let api = testkit.api();
//! let response: CallStatusResponse = reqwest::Client::new()
//!     .get(&api.public_url("api/explorer/v1/call_status/before_transactions"))
//!     .query(&CallStatusQuery {
//!         height: Height(2),
//!         service_id: MyService::INSTANCE_ID,
//!     })
//!     .send()?
//!     .error_for_status()?
//!     .json()?;
//! let err = response.status.0.unwrap_err();
//! assert_eq!(err.description(), "Not a good start");
//! # Ok(())
//! # }
//! ```
//!
//! # Call Status for `after_transactions` hook
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/explorer/v1/call_status/after_transactions` |
//! | Method      | GET   |
//! | Query type  | [`CallStatusQuery`] |
//! | Return type | [`CallStatusResponse`] |
//!
//! Same as the [previous endpoint](#call-status-for-before_transactions-hook), only
//! for a hook executing after all transactions in a block.
//!
//! # Submit Transaction
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/explorer/v1/transactions` |
//! | Method      | POST   |
//! | Query type  | [`TransactionHex`] |
//! | Return type | [`TransactionResponse`] |
//!
//! Adds transaction into the pool of unconfirmed transactions if it is valid
//! and returns an error otherwise.
//!
//! [`TransactionHex`]: struct.TransactionHex.html
//! [`TransactionResponse`]: struct.TransactionResponse.html
//!
//! ```
//! # use exonum::{
//! #     crypto::gen_keypair, helpers::Height, merkledb::{BinaryValue, ObjectHash},
//! #     runtime::ExecutionError,
//! # };
//! # use exonum_rust_runtime::{ExecutionContext, DefaultInstance, Service, ServiceFactory};
//! # use exonum_derive::*;
//! # use exonum_explorer_service::{api::{TransactionHex, TransactionResponse}, ExplorerFactory};
//! # use exonum_testkit::TestKitBuilder;
//! #[exonum_interface]
//! trait ServiceInterface<Ctx> {
//!     type Output;
//!     #[interface_method(id = 0)]
//!     fn do_nothing(&self, ctx: Ctx, _seed: u32) -> Self::Output;
//! }
//!
//! #[derive(Debug, ServiceDispatcher, ServiceFactory)]
//! # #[service_factory(artifact_name = "my-service")]
//! #[service_dispatcher(implements("ServiceInterface"))]
//! struct MyService;
//! // Some implementations skipped for `MyService`...
//! # impl ServiceInterface<ExecutionContext<'_>> for MyService {
//! #    type Output = Result<(), ExecutionError>;
//! #    fn do_nothing(&self, ctx: ExecutionContext<'_>, _seed: u32) -> Self::Output { Ok(()) }
//! # }
//! # impl DefaultInstance for MyService {
//! #     const INSTANCE_ID: u32 = 100;
//! #     const INSTANCE_NAME: &'static str = "my-service";
//! # }
//! # impl Service for MyService {}
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!    .with_default_rust_service(ExplorerFactory)
//!    .with_default_rust_service(MyService)
//!    .build();
//! let tx = gen_keypair().do_nothing(MyService::INSTANCE_ID, 0);
//! let tx_body = hex::encode(tx.to_bytes());
//!
//! let api = testkit.api();
//! let response: TransactionResponse = reqwest::Client::new()
//!     .post(&api.public_url("api/explorer/v1/transactions"))
//!     .json(&TransactionHex { tx_body })
//!     .send()?
//!     .error_for_status()?
//!     .json()?;
//! assert_eq!(response.tx_hash, tx.object_hash());
//! # Ok(())
//! # }
//! ```

pub use exonum_explorer::{
    api::websocket::{
        CommittedTransactionSummary, Notification, SubscriptionType, TransactionFilter,
    },
    api::{
        BlockInfo, BlockQuery, BlocksQuery, BlocksRange, CallStatusQuery, CallStatusResponse,
        TransactionHex, TransactionQuery, TransactionResponse, MAX_BLOCKS_PER_REQUEST,
    },
    TransactionInfo,
};

use exonum::{
    blockchain::{ApiSender, Blockchain, CallInBlock, Schema},
    helpers::Height,
    merkledb::{ObjectHash, Snapshot},
    messages::SignedMessage,
    runtime::ExecutionStatus,
};
use exonum_explorer::{median_precommits_time, BlockchainExplorer};
use exonum_rust_runtime::api::{self, ServiceApiScope};
use futures::{Future, IntoFuture};
use hex::FromHex;
use serde_json::json;

use std::ops::Bound;

pub mod websocket;

/// Exonum blockchain explorer API.
#[derive(Debug, Clone)]
pub(crate) struct ExplorerApi {
    blockchain: Blockchain,
}

impl ExplorerApi {
    /// Creates a new `ExplorerApi` instance.
    pub fn new(blockchain: Blockchain) -> Self {
        Self { blockchain }
    }

    fn blocks(schema: Schema<&dyn Snapshot>, query: BlocksQuery) -> api::Result<BlocksRange> {
        let explorer = BlockchainExplorer::from_schema(schema);
        if query.count > MAX_BLOCKS_PER_REQUEST {
            return Err(api::Error::bad_request()
                .title("Invalid block request")
                .detail(format!(
                    "Max block count per request exceeded ({})",
                    MAX_BLOCKS_PER_REQUEST
                )));
        }

        let (upper, upper_bound) = if let Some(upper) = query.latest {
            if upper > explorer.height() {
                let detail = format!(
                    "Requested latest height {} is greater than the current blockchain height {}",
                    upper,
                    explorer.height()
                );
                return Err(api::Error::not_found()
                    .title("Block not found")
                    .detail(detail));
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

    fn block(schema: Schema<&dyn Snapshot>, query: BlockQuery) -> api::Result<BlockInfo> {
        let explorer = BlockchainExplorer::from_schema(schema);
        explorer.block(query.height).map(From::from).ok_or_else(|| {
            api::Error::not_found()
                .title("Failed to get block info")
                .detail(format!(
                    "Requested block height ({}) exceeds the blockchain height ({})",
                    query.height,
                    explorer.height()
                ))
        })
    }

    fn transaction_info(
        schema: Schema<&dyn Snapshot>,
        query: TransactionQuery,
    ) -> api::Result<TransactionInfo> {
        BlockchainExplorer::from_schema(schema)
            .transaction(&query.hash)
            .ok_or_else(|| {
                let description = serde_json::to_string(&json!({ "type": "unknown" })).unwrap();
                api::Error::not_found()
                    .title("Failed to get transaction info")
                    .detail(description)
            })
    }

    fn transaction_status(
        schema: Schema<&dyn Snapshot>,
        query: TransactionQuery,
    ) -> api::Result<CallStatusResponse> {
        let explorer = BlockchainExplorer::from_schema(schema);

        let tx_info = explorer.transaction(&query.hash).ok_or_else(|| {
            api::Error::not_found()
                .title("Transaction not found")
                .detail(format!("Unknown transaction hash ({})", query.hash))
        })?;

        let tx_info = match tx_info {
            TransactionInfo::Committed(info) => info,
            TransactionInfo::InPool { .. } => {
                let err = api::Error::not_found()
                    .title("Transaction not found")
                    .detail(format!(
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
    fn before_transactions_status(
        schema: Schema<&dyn Snapshot>,
        query: CallStatusQuery,
    ) -> api::Result<CallStatusResponse> {
        let explorer = BlockchainExplorer::from_schema(schema);
        let call_in_block = CallInBlock::before_transactions(query.service_id);
        let status = ExecutionStatus(explorer.call_status(query.height, call_in_block));
        Ok(CallStatusResponse { status })
    }

    /// Returns call status of `after_transactions` hook.
    fn after_transactions_status(
        schema: Schema<&dyn Snapshot>,
        query: CallStatusQuery,
    ) -> api::Result<CallStatusResponse> {
        let explorer = BlockchainExplorer::from_schema(schema);
        let call_in_block = CallInBlock::after_transactions(query.service_id);
        let status = ExecutionStatus(explorer.call_status(query.height, call_in_block));
        Ok(CallStatusResponse { status })
    }

    fn add_transaction(
        snapshot: &dyn Snapshot,
        sender: &ApiSender,
        query: TransactionHex,
    ) -> api::FutureResult<TransactionResponse> {
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
                .broadcast_transaction(verified)
                .map(move |_| TransactionResponse { tx_hash })
                .map_err(|e| api::Error::internal(e).title("Failed to add transaction"))
        };

        Box::new(
            verify_message(snapshot, query.tx_body)
                .into_future()
                .map_err(|e| {
                    api::Error::bad_request()
                        .title("Failed to add transaction to memory pool")
                        .detail(e.to_string())
                })
                .and_then(send_transaction),
        )
    }

    /// Adds explorer API endpoints to the corresponding scope.
    pub fn wire_rest(&self, api_scope: &mut ServiceApiScope) -> &Self {
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
        self
    }
}
