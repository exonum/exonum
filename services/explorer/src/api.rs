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
//! # Transaction Processing
//!
//! This section describes how transactions are processed by the nodes and what the clients
//! can expect when [submitting transactions](#submit-transaction) and
//! [getting transactions](#transaction-by-hash) from the node.
//!
//! As per consensus finality, once a transaction appears in a block, it can never change its
//! status. The "in-block" status is (eventually) shared among all nodes in the network;
//! if an honest Exonum node considers a certain transaction committed, eventually all honest
//! nodes will do the same.
//!
//! At the same time, nodes exhibit *eventual* consistency regarding non-committed transactions
//! (that is, transactions not present in one of the blocks; they are also called *in-pool* transactions).
//! This is true both for the network in general (one node may not know an in-pool transaction
//! known to another node) and, less intuitively, for a single node. The latter means that
//! getting a transaction may return an "not found" error for a small period after the transaction
//! was submitted to the node (aka a *stale read*).
//!
//! The period during which stale reads may exhibit depends on
//! the `mempool.flush_pool_strategy` parameter of the node configuration.
//! This parameter can be adjusted by the nodes independently. With the default value,
//! the coherence period is order of 20 ms.
//!
//! As a consequence of eventual consistency, clients using explorer endpoints **MUST NOT**
//! expect immediate consistency after submitting a transaction. Clients should
//! be prepared that the getter endpoint may return "not found" status after transaction submission.
//! It is recommended that clients poll the getter endpoint with a delay comparable to the coherence
//! period as described above, and poll the endpoint several times if necessary.
//!
//! Note that there may be reasons for such eventual consistency unrelated to node implementation.
//! For example, several Exonum nodes may be placed behind a balancing reverse proxy;
//! in this case, the getter endpoint may be processed by a different node than the one
//! that received a transaction.
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
//! # use exonum_testkit::{Spec, TestKitBuilder};
//! #
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with(Spec::new(ExplorerFactory).with_default_instance())
//!     .build();
//! testkit.create_blocks_until(Height(5));
//!
//! let api = testkit.api();
//! let url = api.public_url("api/explorer/v1/blocks?count=2");
//! let response: BlocksRange = reqwest::get(&url).await?
//!     .error_for_status()?
//!     .json().await?;
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
//! # use exonum_testkit::{Spec, TestKitBuilder};
//! #
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let mut testkit = TestKitBuilder::validator()
//! #    .with(Spec::new(ExplorerFactory).with_default_instance())
//! #    .build();
//! testkit.create_blocks_until(Height(5));
//!
//! let api = testkit.api();
//! let url = api.public_url("api/explorer/v1/block?height=3");
//! let response: BlockInfo = reqwest::get(&url).await?
//!     .error_for_status()?
//!     .json().await?;
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
//! **Important.** See [*Transaction Processing*] section for details about how transactions
//! are processed and which invariants are (not) held during processing.
//!
//! [*Transaction Processing*]: #transaction-processing
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
//! # use exonum_testkit::{Spec, TestKitBuilder};
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
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!    .with(Spec::new(ExplorerFactory).with_default_instance())
//!    .with(Spec::new(MyService).with_default_instance())
//!    .build();
//! let tx = gen_keypair().do_nothing(MyService::INSTANCE_ID, 0);
//! testkit.create_block_with_transaction(tx.clone());
//!
//! let api = testkit.api();
//! let response: TransactionInfo = reqwest::Client::new()
//!     .get(&api.public_url("api/explorer/v1/transactions"))
//!     .query(&TransactionQuery::new(tx.object_hash()))
//!     .send().await?
//!     .error_for_status()?
//!     .json().await?;
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
//! | Query type  | [`TransactionStatusQuery`] |
//! | Return type | [`CallStatusResponse`] |
//!
//! Returns call status of committed transaction.
//!
//! [`TransactionStatusQuery`]: struct.TransactionStatusQuery.html
//! [`CallStatusResponse`]: enum.CallStatusResponse.html
//!
//! ```
//! # use exonum::{
//! #     crypto::gen_keypair, helpers::Height, merkledb::ObjectHash,
//! #     runtime::{ExecutionError, ExecutionFail, ExecutionStatus},
//! # };
//! # use exonum_rust_runtime::{ExecutionContext, DefaultInstance, Service, ServiceFactory};
//! # use exonum_derive::*;
//! # use exonum_explorer_service::{api::TransactionStatusQuery, ExplorerFactory};
//! # use exonum_testkit::{Spec, TestKitBuilder};
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
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!    .with(Spec::new(ExplorerFactory).with_default_instance())
//!    .with(Spec::new(MyService).with_default_instance())
//!    .build();
//! let tx = gen_keypair().cause_error(MyService::INSTANCE_ID, 0);
//! testkit.create_block_with_transaction(tx.clone());
//!
//! let api = testkit.api();
//! let response: ExecutionStatus = reqwest::Client::new()
//!     .get(&api.public_url("api/explorer/v1/call_status/transaction"))
//!     .query(&TransactionStatusQuery::new(tx.object_hash()))
//!     .send().await?
//!     .error_for_status()?
//!     .json().await?;
//! let err = response.0.unwrap_err();
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
//! #     runtime::{ExecutionError, ExecutionFail, ExecutionStatus},
//! # };
//! # use exonum_rust_runtime::{ExecutionContext, DefaultInstance, Service, ServiceFactory};
//! # use exonum_derive::*;
//! # use exonum_explorer_service::{api::CallStatusQuery, ExplorerFactory};
//! # use exonum_testkit::{Spec, TestKitBuilder};
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
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!    .with(Spec::new(ExplorerFactory).with_default_instance())
//!    .with(Spec::new(MyService).with_default_instance())
//!    .build();
//! testkit.create_blocks_until(Height(5));
//!
//! let api = testkit.api();
//! let response: ExecutionStatus = reqwest::Client::new()
//!     .get(&api.public_url("api/explorer/v1/call_status/before_transactions"))
//!     .query(&CallStatusQuery::new(Height(2), MyService::INSTANCE_ID))
//!     .send().await?
//!     .error_for_status()?
//!     .json().await?;
//! let err = response.0.unwrap_err();
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
//! **Important.** See [*Transaction Processing*] section for details about how transactions
//! are processed and which invariants are (not) held during processing.
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
//! # use exonum_testkit::{Spec, TestKitBuilder};
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
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!    .with(Spec::new(ExplorerFactory).with_default_instance())
//!    .with(Spec::new(MyService).with_default_instance())
//!    .build();
//! let tx = gen_keypair().do_nothing(MyService::INSTANCE_ID, 0);
//!
//! let api = testkit.api();
//! let response: TransactionResponse = reqwest::Client::new()
//!     .post(&api.public_url("api/explorer/v1/transactions"))
//!     .json(&TransactionHex::new(&tx))
//!     .send().await?
//!     .error_for_status()?
//!     .json().await?;
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
        TransactionHex, TransactionQuery, TransactionResponse, TransactionStatusQuery,
        MAX_BLOCKS_PER_REQUEST,
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
use exonum_explorer::BlockchainExplorer;
use exonum_rust_runtime::api::{self, ServiceApiScope};
use futures::{future, Future, FutureExt, TryFutureExt};
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

    fn blocks(schema: Schema<&dyn Snapshot>, query: &BlocksQuery) -> api::Result<BlocksRange> {
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
            .map(|block| BlockInfo::summary(block, query))
            .collect();

        let height = if blocks.len() < query.count {
            query.earliest.unwrap_or(Height(0))
        } else {
            blocks.last().map_or(Height(0), |info| info.block.height)
        };

        Ok(BlocksRange::new(height..upper.next(), blocks))
    }

    fn block(schema: Schema<&dyn Snapshot>, query: &BlockQuery) -> api::Result<BlockInfo> {
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
        query: &TransactionQuery,
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

    fn get_status(
        schema: &Schema<&dyn Snapshot>,
        block_height: Height,
        call_in_block: CallInBlock,
        with_proof: bool,
    ) -> api::Result<CallStatusResponse> {
        let records = schema.call_records(block_height).ok_or_else(|| {
            api::Error::not_found()
                .title("Block not found")
                .detail(format!(
                    "Block with height {} is not yet created",
                    block_height
                ))
        })?;

        Ok(if with_proof {
            let proof = records.get_proof(call_in_block);
            CallStatusResponse::Proof(proof)
        } else {
            let status = ExecutionStatus(records.get(call_in_block));
            CallStatusResponse::Simple(status)
        })
    }

    fn transaction_status(
        schema: &Schema<&dyn Snapshot>,
        query: &TransactionStatusQuery,
    ) -> api::Result<CallStatusResponse> {
        let tx_location = schema
            .transactions_locations()
            .get(&query.hash)
            .ok_or_else(|| {
                api::Error::not_found()
                    .title("Transaction not committed")
                    .detail(format!("Unknown transaction hash ({})", query.hash))
            })?;

        let call_in_block = CallInBlock::transaction(tx_location.position_in_block());
        let block_height = tx_location.block_height();
        Self::get_status(schema, block_height, call_in_block, query.with_proof)
    }

    /// Returns call status of `before_transactions` hook.
    fn before_transactions_status(
        schema: &Schema<&dyn Snapshot>,
        query: &CallStatusQuery,
    ) -> api::Result<CallStatusResponse> {
        let call_in_block = CallInBlock::before_transactions(query.service_id);
        Self::get_status(schema, query.height, call_in_block, query.with_proof)
    }

    /// Returns call status of `after_transactions` hook.
    fn after_transactions_status(
        schema: &Schema<&dyn Snapshot>,
        query: &CallStatusQuery,
    ) -> api::Result<CallStatusResponse> {
        let call_in_block = CallInBlock::after_transactions(query.service_id);
        Self::get_status(schema, query.height, call_in_block, query.with_proof)
    }

    fn add_transaction(
        snapshot: &dyn Snapshot,
        sender: &ApiSender,
        query: TransactionHex,
    ) -> impl Future<Output = api::Result<TransactionResponse>> {
        // Synchronous part of message verification.
        let verify_message = |snapshot: &dyn Snapshot, hex: String| -> anyhow::Result<_> {
            let msg = SignedMessage::from_hex(hex)?;
            let tx_hash = msg.object_hash();
            let verified = msg.into_verified()?;
            Blockchain::check_tx(snapshot, &verified)?;
            Ok((verified, tx_hash))
        };

        let (verified, tx_hash) = match verify_message(snapshot, query.tx_body) {
            Ok((verified, tx_hash)) => (verified, tx_hash),
            Err(err) => {
                let err = api::Error::bad_request()
                    .title("Failed to add transaction to memory pool")
                    .detail(err.to_string());
                return future::err(err).left_future();
            }
        };

        sender
            .broadcast_transaction(verified)
            .map_ok(move |_| TransactionResponse::new(tx_hash))
            .map_err(|err| api::Error::internal(err).title("Failed to add transaction"))
            .right_future()
    }

    /// Adds explorer API endpoints to the corresponding scope.
    pub fn wire_rest(&self, api_scope: &mut ServiceApiScope) -> &Self {
        api_scope
            .endpoint("v1/blocks", |state, query| {
                future::ready(Self::blocks(state.data().for_core(), &query))
            })
            .endpoint("v1/block", |state, query| {
                future::ready(Self::block(state.data().for_core(), &query))
            })
            .endpoint("v1/call_status/transaction", |state, query| {
                future::ready(Self::transaction_status(&state.data().for_core(), &query))
            })
            .endpoint("v1/call_status/after_transactions", |state, query| {
                future::ready(Self::after_transactions_status(
                    &state.data().for_core(),
                    &query,
                ))
            })
            .endpoint("v1/call_status/before_transactions", |state, query| {
                future::ready(Self::before_transactions_status(
                    &state.data().for_core(),
                    &query,
                ))
            })
            .endpoint("v1/transactions", |state, query| {
                future::ready(Self::transaction_info(state.data().for_core(), &query))
            });

        let tx_sender = self.blockchain.sender().to_owned();
        api_scope.endpoint_mut("v1/transactions", move |state, query| {
            Self::add_transaction(state.snapshot(), &tx_sender, query)
        });
        self
    }
}
