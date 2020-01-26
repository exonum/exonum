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

//! Types used by the testkit server.
//!
//! The server may be created via [`serve`] method in `TestKitBuilder`. Testkit-specific
//! server endpoints are documented below. Other endpoints exposed by the server are the same
//! as for an Exonum node or [`TestKitApi`]; that is, the server exposes HTTP API of Exonum
//! services and node plugins.
//!
//! # HTTP endpoints
//!
//! All endpoints are served on the private HTTP server, which listens on the second
//! address passed to `TestKitBuilder::serve()`.
//!
//! ## Testkit status
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/testkit/v1/status` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | [`TestKitStatus`] |
//!
//! Outputs the status of the testkit, which includes:
//!
//! - Current blockchain height
//! - Current test network configuration
//!
//! [`TestKitStatus`]: struct.TestKitStatus.html
//!
//! ## Create block
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/testkit/v1/blocks/create` |
//! | Method      | POST  |
//! | Body type   | [`CreateBlock`] |
//! | Return type | `BlockWithTransactions` |
//!
//! Creates a new block in the testkit blockchain. If the
//! JSON body of the request is an empty object, the call is functionally equivalent
//! to [`create_block`]. Otherwise, if the body has the `tx_hashes` field specifying an array
//! of transaction hashes, the call is equivalent to [`create_block_with_tx_hashes`] supplied
//! with these hashes.
//!
//! Returns the latest block from the blockchain on success.
//!
//! [`CreateBlock`]: struct.CreateBlock.html
//!
//! ## Roll back
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/testkit/v1/blocks/rollback` |
//! | Method      | POST  |
//! | Body type   | `Height` |
//! | Return type | `BlockWithTransactions` |
//!
//! Acts as a rough [`rollback`] equivalent. The blocks are rolled back up and including the block
//! at the specified in JSON body `height` value (a positive integer), so that after the request
//! the blockchain height is equal to `height - 1`. If the specified height is greater than the
//! blockchain height, the request performs no action.
//!
//! Returns the latest block from the blockchain on success.
//!
//! [`serve`]: ../struct.TestKitBuilder.html#method.serve
//! [`TestKitApi`]: ../struct.TestKitApi.html
//! [`create_block`]: ../struct.TestKit.html#method.create_block
//! [`create_block_with_tx_hashes`]: ../struct.TestKit.html#method.create_block_with_tx_hashes
//! [`rollback`]: ../struct.TestKit.html#method.rollback

use actix::prelude::*;
use exonum::{blockchain::ConsensusConfig, crypto::Hash, helpers::Height};
use exonum_api::{self as api, ApiAggregator, ApiBuilder};
use exonum_explorer::{BlockWithTransactions, BlockchainExplorer};
use futures::{sync::oneshot, Future};
use serde::{Deserialize, Serialize};

use std::thread::{self, JoinHandle};

use super::TestKit;

#[derive(Debug)]
pub(crate) struct TestKitActor(TestKit);

impl TestKitActor {
    pub(crate) fn spawn(mut testkit: TestKit) -> (ApiAggregator, JoinHandle<i32>) {
        let mut api_aggregator = testkit.update_aggregator();

        // Spawn the testkit actor on the new `actix` system.
        let (actor_tx, actor_rx) = oneshot::channel();
        let join_handle = thread::spawn(|| {
            let system = System::new("testkit");
            let testkit = Self(testkit).start();
            actor_tx.send(testkit).unwrap();
            system.run()
        });

        let testkit = actor_rx.wait().expect("Failed spawning testkit server");
        api_aggregator.insert("testkit", Self::api(testkit));
        (api_aggregator, join_handle)
    }

    fn api(addr: Addr<Self>) -> ApiBuilder {
        let mut builder = ApiBuilder::new();
        let api_scope = builder.private_scope();

        let addr_ = addr.clone();
        api_scope.endpoint("v1/status", move |()| {
            Box::new(addr_.send(GetStatus).then(flatten_err)) as api::FutureResult<_>
        });
        let addr_ = addr.clone();
        api_scope.endpoint_mut("v1/blocks/rollback", move |height| {
            Box::new(addr_.send(RollBack(height)).then(flatten_err)) as api::FutureResult<_>
        });
        let addr_ = addr.clone();
        api_scope.endpoint_mut("v1/blocks/create", move |query: CreateBlock| {
            Box::new(addr_.send(query).then(flatten_err)) as api::FutureResult<_>
        });
        builder
    }
}

impl Actor for TestKitActor {
    type Context = Context<Self>;
}

fn flatten_err<T>(res: Result<Result<T, api::Error>, MailboxError>) -> Result<T, api::Error> {
    match res {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(api::Error::internal(e)),
    }
}

#[derive(Debug)]
struct GetStatus;

impl Message for GetStatus {
    type Result = api::Result<TestKitStatus>;
}

/// Testkit status, returned by the corresponding API endpoint.
#[derive(Debug, Serialize, Deserialize)]
pub struct TestKitStatus {
    /// Current blockchain height.
    pub height: Height,
    /// Currently active network configuration.
    pub configuration: ConsensusConfig,
}

impl Handler<GetStatus> for TestKitActor {
    type Result = api::Result<TestKitStatus>;

    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(TestKitStatus {
            height: self.0.height(),
            configuration: self.0.consensus_config(),
        })
    }
}

/// Block creation parameters for the testkit server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBlock {
    /// List of transaction hashes to include in the block. Transactions should be
    /// present in the memory pool of the testkit.
    ///
    /// If the field is set to `None` (e.g., omitted in a `POST` request to the server),
    /// the server will create a block with all transactions from the memory pool.
    #[serde(default)]
    pub tx_hashes: Option<Vec<Hash>>,
}

impl Message for CreateBlock {
    type Result = api::Result<BlockWithTransactions>;
}

impl Handler<CreateBlock> for TestKitActor {
    type Result = api::Result<BlockWithTransactions>;

    fn handle(&mut self, msg: CreateBlock, _ctx: &mut Self::Context) -> Self::Result {
        let block_info = if let Some(tx_hashes) = msg.tx_hashes {
            let maybe_missing_tx = tx_hashes.iter().find(|h| !self.0.is_tx_in_pool(h));
            if let Some(missing_tx) = maybe_missing_tx {
                return Err(api::Error::bad_request()
                    .title("Creating block failed")
                    .detail(format!(
                        "Transaction not in mempool: {}",
                        missing_tx.to_string()
                    )));
            }

            // NB: checkpoints must correspond 1-to-1 to blocks.
            self.0.checkpoint();
            self.0.create_block_with_tx_hashes(&tx_hashes)
        } else {
            self.0.checkpoint();
            self.0.create_block()
        };
        Ok(block_info)
    }
}

#[derive(Debug)]
struct RollBack(Height);

impl Message for RollBack {
    type Result = api::Result<Option<BlockWithTransactions>>;
}

impl Handler<RollBack> for TestKitActor {
    type Result = api::Result<Option<BlockWithTransactions>>;

    fn handle(&mut self, RollBack(height): RollBack, _ctx: &mut Self::Context) -> Self::Result {
        if height == Height(0) {
            return Err(api::Error::bad_request().title("Cannot rollback past genesis block"));
        }

        if self.0.height() >= height {
            let rollback_blocks = (self.0.height().0 - height.0 + 1) as usize;
            for _ in 0..rollback_blocks {
                self.0.rollback();
            }
        }

        let snapshot = self.0.snapshot();
        let explorer = BlockchainExplorer::new(snapshot.as_ref());
        Ok(explorer.block_with_txs(self.0.height()))
    }
}

#[cfg(test)]
mod tests {
    use exonum::{
        crypto::{gen_keypair, Hash},
        helpers::Height,
        messages::{AnyTx, Verified},
        runtime::{ExecutionContext, ExecutionError},
    };
    use exonum_derive::{exonum_interface, ServiceDispatcher, ServiceFactory};
    use exonum_explorer::BlockWithTransactions;
    use exonum_merkledb::ObjectHash;
    use exonum_rust_runtime::{api, Service, ServiceFactory};
    use pretty_assertions::assert_eq;

    use std::time::Duration;

    use super::*;
    use crate::{TestKitApi, TestKitBuilder};

    const TIMESTAMP_SERVICE_ID: u32 = 2;
    const TIMESTAMP_SERVICE_NAME: &str = "sample";

    fn timestamp(s: &str) -> Verified<AnyTx> {
        gen_keypair().timestamp(TIMESTAMP_SERVICE_ID, s.to_owned())
    }

    #[derive(Debug, ServiceDispatcher, ServiceFactory)]
    #[service_factory(artifact_name = "sample-service")]
    #[service_dispatcher(implements("SampleInterface"))]
    struct SampleService;

    #[exonum_interface(auto_ids)]
    trait SampleInterface<Ctx> {
        type Output;
        fn timestamp(&self, ctx: Ctx, arg: String) -> Self::Output;
    }

    impl SampleInterface<ExecutionContext<'_>> for SampleService {
        type Output = Result<(), ExecutionError>;

        fn timestamp(&self, _ctx: ExecutionContext<'_>, _arg: String) -> Self::Output {
            Ok(())
        }
    }

    impl Service for SampleService {}

    /// Initializes testkit, passes it into a handler, and creates the specified number
    /// of empty blocks in the testkit blockchain.
    fn init_handler(height: Height) -> TestKitApi {
        let service = SampleService;
        let artifact = service.artifact_id();
        let mut testkit = TestKitBuilder::validator()
            .with_artifact(artifact.clone())
            .with_instance(
                artifact.into_default_instance(TIMESTAMP_SERVICE_ID, TIMESTAMP_SERVICE_NAME),
            )
            .with_rust_service(service)
            .build();
        testkit.create_blocks_until(height);
        // Process incoming events in background.
        let events = testkit.remove_events_stream();
        thread::spawn(|| events.wait().ok());

        let api_sender = testkit.api_sender.clone();
        let (aggregator, _) = TestKitActor::spawn(testkit);
        TestKitApi::from_raw_parts(aggregator, api_sender)
    }

    fn sleep() {
        thread::sleep(Duration::from_millis(20));
    }

    #[test]
    fn test_create_block_with_empty_body() {
        let api = init_handler(Height(0));
        let tx = timestamp("foo");
        api.send(tx.clone());
        sleep();

        // Test a bodiless request
        let block_info: BlockWithTransactions = api
            .private("api/testkit")
            .query(&CreateBlock { tx_hashes: None })
            .post("v1/blocks/create")
            .unwrap();

        assert_eq!(block_info.header.height, Height(1));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(block_info.transactions[0].message(), &tx);

        let block_info: BlockWithTransactions = api
            .private("api/testkit")
            .query(&Height(1))
            .post("v1/blocks/rollback")
            .unwrap();
        assert_eq!(block_info.header.height, Height(0));
        api.send(tx.clone());
        sleep();

        let block_info: BlockWithTransactions = api
            .private("api/testkit")
            .query(&CreateBlock { tx_hashes: None })
            .post("v1/blocks/create")
            .unwrap();
        assert_eq!(block_info.header.height, Height(1));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(block_info.transactions[0].message(), &tx);
    }

    #[test]
    fn test_create_block_with_specified_transactions() {
        let api = init_handler(Height(0));
        let tx_foo = timestamp("foo");
        let tx_bar = timestamp("bar");
        api.send(tx_foo.clone());
        api.send(tx_bar.clone());
        sleep();

        let body = CreateBlock {
            tx_hashes: Some(vec![tx_foo.object_hash()]),
        };
        let block_info: BlockWithTransactions = api
            .private("api/testkit")
            .query(&body)
            .post("v1/blocks/create")
            .unwrap();
        assert_eq!(block_info.header.height, Height(1));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(block_info.transactions[0].message(), &tx_foo);

        let body = CreateBlock {
            tx_hashes: Some(vec![tx_bar.object_hash()]),
        };
        let block_info: BlockWithTransactions = api
            .private("api/testkit")
            .query(&body)
            .post("v1/blocks/create")
            .unwrap();
        assert_eq!(block_info.header.height, Height(2));
        assert_eq!(block_info.transactions.len(), 1);
        assert_eq!(block_info.transactions[0].message(), &tx_bar);
    }

    #[test]
    fn test_create_block_with_bogus_transaction() {
        let api = init_handler(Height(0));

        let body = CreateBlock {
            tx_hashes: Some(vec![Hash::zero()]),
        };
        let err = api
            .private("api/testkit")
            .query(&body)
            .post::<BlockWithTransactions>("v1/blocks/create")
            .unwrap_err();

        assert_eq!(err.http_code, api::HttpStatusCode::BAD_REQUEST);
        assert_eq!(err.body.title, "Creating block failed");
        assert_eq!(
            err.body.detail,
            format!("Transaction not in mempool: {}", Hash::zero())
        );
    }

    #[test]
    fn test_rollback_normal() {
        let api = init_handler(Height(0));

        for i in 0..4 {
            let block: BlockWithTransactions = api
                .private("api/testkit")
                .query(&CreateBlock { tx_hashes: None })
                .post("v1/blocks/create")
                .unwrap();
            assert_eq!(block.height(), Height(i + 1));
        }

        // Test that requests with "overflowing" heights do nothing
        let block_info: BlockWithTransactions = api
            .private("api/testkit")
            .query(&Height(10))
            .post("v1/blocks/rollback")
            .unwrap();
        assert_eq!(block_info.header.height, Height(4));

        // Test idempotence of the rollback endpoint
        for _ in 0..2 {
            let block_info: BlockWithTransactions = api
                .private("api/testkit")
                .query(&Height(4))
                .post("v1/blocks/rollback")
                .unwrap();

            assert_eq!(block_info.header.height, Height(3));
        }

        // Test roll-back to the genesis block
        let block = api
            .private("api/testkit")
            .query(&Height(1))
            .post::<BlockWithTransactions>("v1/blocks/rollback")
            .unwrap();
        assert_eq!(block.header.height, Height(0));
    }

    #[test]
    fn test_rollback_past_genesis() {
        let api = init_handler(Height(4));
        let err = api
            .private("api/testkit")
            .query(&Height(0))
            .post::<BlockWithTransactions>("v1/blocks/rollback")
            .unwrap_err();

        assert_eq!(err.http_code, api::HttpStatusCode::BAD_REQUEST);
        assert_eq!(err.body.title, "Cannot rollback past genesis block");
    }
}
