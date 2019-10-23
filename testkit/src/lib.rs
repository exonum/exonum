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

//! Testkit for Exonum blockchain framework, allowing to test service APIs synchronously
//! and in the same process as the testkit.
//!
//! # Example
//! ```
//! use exonum::{
//!     runtime::{InstanceDescriptor, rust::{Transaction, TransactionContext, Service}},
//!     blockchain::{Block, Schema, ExecutionError, InstanceCollection},
//!     crypto::{gen_keypair, Hash},
//!     explorer::TransactionInfo,
//!     helpers::Height,
//!     api::node::public::explorer::{BlocksQuery, BlocksRange, TransactionQuery},
//! };
//! use serde_derive::{Serialize, Deserialize};
//! use exonum_derive::{exonum_service, ServiceFactory, ProtobufConvert};
//! use exonum_merkledb::{ObjectHash, Snapshot};
//! use exonum_testkit::{txvec, ApiKind, TestKitBuilder};
//!
//! // Simple service implementation.
//!
//! const SERVICE_ID: u32 = 1;
//!
//! #[derive(Debug, Clone, Serialize, Deserialize, ProtobufConvert)]
//! #[exonum(pb = "exonum_testkit::proto::examples::TxTimestamp")]
//! pub struct TxTimestamp {
//!     message: String,
//! }
//!
//! #[derive(Clone, Default, Debug, ServiceFactory)]
//! #[exonum(
//!     artifact_name = "timestamping",
//!     artifact_version = "1.0.0",
//!     proto_sources = "exonum_testkit::proto",
//!     implements("TimestampingInterface")
//! )]
//! struct TimestampingService;
//!
//! impl Service for TimestampingService {
//!     fn state_hash(&self, _: InstanceDescriptor, _: &dyn Snapshot) -> Vec<Hash> { vec![] }
//! }
//!
//! #[exonum_service]
//! pub trait TimestampingInterface {
//!     fn timestamp(&self, _: TransactionContext, arg: TxTimestamp) -> Result<(), ExecutionError>;
//! }
//!
//! impl TimestampingInterface for TimestampingService {
//!     fn timestamp(&self, _: TransactionContext, arg: TxTimestamp) -> Result<(), ExecutionError> {
//!         Ok(())
//!     }
//! }
//!
//! fn main() {
//!     // Create testkit for network with four validators.
//!     let mut testkit = TestKitBuilder::validator()
//!         .with_validators(4)
//!         .with_rust_service(
//!             InstanceCollection::new(TimestampingService)
//!                 .with_instance(SERVICE_ID, "timestamping", ())
//!         )
//!         .create();
//!
//!     // Create few transactions.
//!     let keys = gen_keypair();
//!     let id = SERVICE_ID;
//!     let tx1 = TxTimestamp { message: "Down To Earth".into() }.sign(id, keys.0, &keys.1);
//!     let tx2 = TxTimestamp { message: "Cry Over Spilt Milk".into() }.sign(id, keys.0, &keys.1);
//!     let tx3 = TxTimestamp { message: "Dropping Like Flies".into() }.sign(id, keys.0, &keys.1);
//!     // Commit them into blockchain.
//!     testkit.create_block_with_transactions(txvec![
//!         tx1.clone(), tx2.clone(), tx3.clone()
//!     ]);
//!
//!     // Add a single transaction.
//!     let tx4 = TxTimestamp { message: "Barking up the wrong tree".into() }.sign(id, keys.0, &keys.1);
//!     testkit.create_block_with_transaction(tx4.clone());
//!
//!     // Check results with schema.
//!     let snapshot = testkit.snapshot();
//!     let schema = Schema::new(&snapshot);
//!     assert!(schema.transactions().contains(&tx1.object_hash()));
//!     assert!(schema.transactions().contains(&tx2.object_hash()));
//!     assert!(schema.transactions().contains(&tx3.object_hash()));
//!     assert!(schema.transactions().contains(&tx4.object_hash()));
//!
//!     // Check results with api.
//!     let api = testkit.api();
//!     let explorer_api = api.public(ApiKind::Explorer);
//!     let response: BlocksRange = explorer_api
//!         .query(&BlocksQuery {
//!             count: 10,
//!             ..Default::default()
//!         })
//!         .get("v1/blocks")
//!         .unwrap();
//!     let (blocks, range) = (response.blocks, response.range);
//!     assert_eq!(blocks.len(), 3);
//!     assert_eq!(range.start, Height(0));
//!     assert_eq!(range.end, Height(3));
//!
//!     let info = explorer_api
//!         .query(&TransactionQuery::new(tx1.object_hash()))
//!         .get::<TransactionInfo>("v1/transactions")
//!         .unwrap();
//! }
//! ```

//#![warn(missing_debug_implementations, missing_docs)]
#![deny(unsafe_code, bare_trait_objects)]

#[cfg_attr(test, macro_use)]
#[cfg(test)]
extern crate assert_matches;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[cfg_attr(test, macro_use)]
#[cfg(test)]
extern crate exonum_derive;

pub use crate::{
    api::{ApiKind, TestKitApi},
    builder::{InstanceCollection, TestKitBuilder},
    compare::ComparableSnapshot,
    network::{TestNetwork, TestNode},
    server::TestKitStatus,
};
pub mod compare;
pub mod proto;
pub mod simple_supervisor;

use exonum::{
    api::{
        backends::actix::{ApiRuntimeConfig, SystemRuntimeConfig},
        ApiAccess,
    },
    blockchain::{
        Blockchain, BlockchainBuilder, ConsensusConfig, InstanceConfig, Schema as CoreSchema,
    },
    crypto::{self, Hash},
    explorer::{BlockWithTransactions, BlockchainExplorer},
    helpers::{byzantine_quorum, Height, ValidatorId},
    merkledb::{BinaryValue, Database, ObjectHash, Snapshot, TemporaryDB},
    messages::{AnyTx, Verified},
    node::{ApiSender, ExternalMessage},
    runtime::{rust::ServiceFactory, InstanceId, Runtime},
};
use futures::{sync::mpsc, Future, Stream};
use tokio_core::reactor::Core;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, RwLock},
    {fmt, net::SocketAddr},
};

use crate::{
    checkpoint_db::{CheckpointDb, CheckpointDbHandler},
    poll_events::poll_events,
};

#[macro_use]
mod macros;
mod api;
mod builder;
mod checkpoint_db;
mod network;
mod poll_events;
mod server;

/// Testkit for testing blockchain services. It offers simple network configuration emulation
/// (with no real network setup).
pub struct TestKit {
    blockchain: Blockchain,
    db_handler: CheckpointDbHandler<TemporaryDB>,
    events_stream: Box<dyn Stream<Item = (), Error = ()> + Send + Sync>,
    processing_lock: Arc<Mutex<()>>,
    network: TestNetwork,
    api_sender: ApiSender,
}

impl fmt::Debug for TestKit {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TestKit")
            .field("blockchain", &self.blockchain)
            .field("network", &self.network)
            .finish()
    }
}

impl TestKit {
    /// Creates a new `TestKit` with a single validator with the given Rust service.
    pub fn for_rust_service(
        service_factory: impl Into<Box<dyn ServiceFactory>>,
        name: impl Into<String>,
        id: InstanceId,
        constructor: impl BinaryValue,
    ) -> Self {
        TestKitBuilder::validator()
            .with_rust_service(InstanceCollection::new(service_factory).with_instance(
                id,
                name,
                constructor,
            ))
            .create()
    }

    fn assemble(
        database: impl Into<CheckpointDb<TemporaryDB>>,
        network: TestNetwork,
        genesis: ConsensusConfig,
        runtimes: impl IntoIterator<Item = (u32, Box<dyn Runtime>)>,
        instances: impl IntoIterator<Item = InstanceConfig>,
    ) -> Self {
        let api_channel = mpsc::channel(1_000);
        let api_sender = ApiSender::new(api_channel.0.clone());
        let db = database.into();
        let db_handler = db.handler();
        let mut builder = BlockchainBuilder::new(db, genesis, network.us().service_keypair());

        for runtime in runtimes {
            builder = builder.with_additional_runtime(runtime);
        }

        let blockchain = builder
            .with_builtin_instances(instances)
            .finalize(api_sender.clone(), mpsc::channel(0).0)
            .expect("Unable to create blockchain instance");

        let processing_lock = Arc::new(Mutex::new(()));
        let processing_lock_ = Arc::clone(&processing_lock);

        let events_stream: Box<dyn Stream<Item = (), Error = ()> + Send + Sync> = {
            let mut blockchain = blockchain.clone();
            Box::new(api_channel.1.and_then(move |event| {
                let guard = processing_lock_.lock().unwrap();
                let fork = blockchain.fork();
                let mut schema = CoreSchema::new(&fork);
                match event {
                    ExternalMessage::Transaction(tx) => {
                        let hash = tx.object_hash();
                        if !schema.transactions().contains(&hash) {
                            schema.add_transaction_into_pool(tx.clone());
                        }
                    }
                    ExternalMessage::PeerAdd(_)
                    | ExternalMessage::Enable(_)
                    | ExternalMessage::Shutdown => { /* Ignored */ }
                }
                blockchain.merge(fork.into_patch()).unwrap();
                drop(guard);
                Ok(())
            }))
        };

        TestKit {
            blockchain,
            db_handler,
            api_sender,
            events_stream,
            processing_lock,
            network,
        }
    }

    /// Creates an instance of `TestKitApi` to test the API provided by services.
    pub fn api(&self) -> TestKitApi {
        TestKitApi::new(self)
    }

    /// Polls the *existing* events from the event loop until exhaustion. Does not wait
    /// until new events arrive.
    pub fn poll_events(&mut self) {
        poll_events(&mut self.events_stream);
    }

    /// Returns a snapshot of the current blockchain state.
    pub fn snapshot(&self) -> Box<dyn Snapshot> {
        self.blockchain.snapshot()
    }

    /// Returns a blockchain used by the testkit.
    pub fn blockchain(&self) -> Blockchain {
        self.blockchain.clone()
    }

    /// Sets a checkpoint for a future [`rollback`](#method.rollback).
    pub fn checkpoint(&mut self) {
        self.db_handler.checkpoint()
    }

    /// Rolls the blockchain back to the latest [`checkpoint`](#method.checkpoint).
    ///
    /// # Examples
    ///
    /// Rollbacks are useful in testing alternative scenarios (e.g., transactions executed
    /// in different order and/or in different blocks) that require an expensive setup:
    ///
    /// ```
    /// # use serde_derive::{Serialize, Deserialize};
    /// # use exonum_derive::{exonum_service, ServiceFactory, ProtobufConvert};
    /// # use exonum_testkit::{txvec, TestKit, TestKitBuilder};
    /// # use exonum_merkledb::Snapshot;
    /// # use exonum::{
    /// #     blockchain::{ExecutionError, InstanceCollection},
    /// #     crypto::{PublicKey, Hash, SecretKey},
    /// #     runtime::{InstanceDescriptor, rust::{Transaction, TransactionContext, Service}},
    /// # };
    /// #
    /// # const SERVICE_ID: u32 = 1;
    /// #
    /// # #[derive(Clone, Default, Debug, ServiceFactory)]
    /// # #[exonum(
    /// #     artifact_name = "my",
    /// #     artifact_version = "1.0.0",
    /// #     proto_sources = "exonum_testkit::proto",
    /// #     implements("MyInterface")
    /// # )]
    /// # pub struct MyService;
    /// #
    /// # impl Service for MyService {
    /// #     fn state_hash(&self, _: InstanceDescriptor, _: &dyn Snapshot) -> Vec<Hash> { vec![] }
    /// # }
    /// #
    /// # #[exonum_service]
    /// # pub trait MyInterface {
    /// #     fn my_tx(&self, _: TransactionContext, arg: MyTransaction) -> Result<(), ExecutionError>;
    /// # }
    /// #
    /// # impl MyInterface for MyService {
    /// #     fn my_tx(&self, _: TransactionContext, arg: MyTransaction) -> Result<(), ExecutionError> {
    /// #         Ok(())
    /// #     }
    /// # }
    /// #
    /// # #[derive(Debug, Clone, Serialize, Deserialize, ProtobufConvert)]
    /// # #[exonum(pb = "exonum_testkit::proto::examples::TxTimestamp")]
    /// # pub struct MyTransaction {
    /// #     message: String,
    /// # }
    /// #
    /// # fn expensive_setup(_: &mut TestKit) {}
    /// # fn assert_something_about(_: &TestKit) {}
    /// #
    /// # fn main() {
    /// let mut testkit = TestKitBuilder::validator()
    ///     .with_rust_service(
    ///         InstanceCollection::new(MyService)
    ///            .with_instance(SERVICE_ID, "my", ())
    ///     )
    ///     .create();
    /// expensive_setup(&mut testkit);
    /// let (pubkey, key) = exonum::crypto::gen_keypair();
    /// let tx_a = MyTransaction { message: "foo".into() }.sign(SERVICE_ID, pubkey, &key);
    /// let tx_b = MyTransaction { message: "bar".into() }.sign(SERVICE_ID, pubkey, &key);
    ///
    /// testkit.checkpoint();
    /// testkit.create_block_with_transactions(txvec![tx_a.clone(), tx_b.clone()]);
    /// assert_something_about(&testkit);
    /// testkit.rollback();
    ///
    /// testkit.checkpoint();
    /// testkit.create_block_with_transactions(txvec![tx_a.clone()]);
    /// testkit.create_block_with_transactions(txvec![tx_b.clone()]);
    /// assert_something_about(&testkit);
    /// testkit.rollback();
    /// # }
    /// ```
    pub fn rollback(&mut self) {
        self.db_handler.rollback()
    }

    /// Executes a list of transactions given the current state of the blockchain, but does not
    /// commit execution results to the blockchain. The execution result is the same
    /// as if transactions were included into a new block; for example,
    /// transactions included into one of previous blocks do not lead to any state changes.
    pub fn probe_all<I>(&mut self, transactions: I) -> Box<dyn Snapshot>
    where
        I: IntoIterator<Item = Verified<AnyTx>>,
    {
        self.poll_events();
        // Filter out already committed transactions; otherwise,
        // `create_block_with_transactions()` will panic.
        let snapshot = self.snapshot();
        let schema = CoreSchema::new(&snapshot);
        let uncommitted_txs = transactions.into_iter().filter(|tx| {
            !schema.transactions().contains(&tx.object_hash())
                || schema.transactions_pool().contains(&tx.object_hash())
        });

        self.checkpoint();
        self.create_block_with_transactions(uncommitted_txs);
        let snapshot = self.snapshot();
        self.rollback();
        snapshot
    }

    /// Executes a transaction given the current state of the blockchain but does not
    /// commit execution results to the blockchain. The execution result is the same
    /// as if a transaction was included into a new block; for example,
    /// a transaction included into one of previous blocks does not lead to any state changes.
    pub fn probe(&mut self, transaction: Verified<AnyTx>) -> Box<dyn Snapshot> {
        self.probe_all(vec![transaction])
    }

    fn do_create_block(&mut self, tx_hashes: &[Hash]) -> BlockWithTransactions {
        let new_block_height = self.height().next();
        let last_hash = self.last_block_hash();
        let saved_consensus_config = self.consensus_config();

        let (block_hash, patch) = {
            let validator_id = self.leader().validator_id().unwrap();
            self.blockchain.create_patch(
                validator_id,
                new_block_height,
                tx_hashes,
                &mut BTreeMap::new(),
            )
        };

        let propose =
            self.leader()
                .create_propose(new_block_height, last_hash, tx_hashes.iter().cloned());
        let precommits: Vec<_> = self
            .network()
            .validators()
            .iter()
            .map(|v| v.create_precommit(propose.as_ref(), block_hash))
            .collect();

        let guard = self.processing_lock.lock().unwrap();
        self.blockchain
            .commit(
                patch,
                block_hash,
                precommits.into_iter(),
                &mut BTreeMap::new(),
            )
            .unwrap();
        drop(guard);

        // Modify the self configuration
        let actual_consensus_config = self.consensus_config();
        if actual_consensus_config != saved_consensus_config {
            self.network_mut()
                .update_consensus_config(actual_consensus_config);
        }

        self.poll_events();

        let snapshot = self.snapshot();
        BlockchainExplorer::new(snapshot.as_ref())
            .block_with_txs(self.height())
            .unwrap()
    }

    /// Creates a block with the given transactions.
    /// Transactions that are in the pool will be ignored.
    ///
    /// # Return value
    ///
    /// Returns information about the created block.
    ///
    /// # Panics
    ///
    /// - Panics if any of transactions has been already committed to the blockchain.
    pub fn create_block_with_transactions<I>(&mut self, txs: I) -> BlockWithTransactions
    where
        I: IntoIterator<Item = Verified<AnyTx>>,
    {
        let tx_hashes: Vec<_> = {
            let fork = self.blockchain.fork();
            let hashes = {
                let mut schema = CoreSchema::new(&fork);

                txs.into_iter()
                    .map(|tx| {
                        let tx_id = tx.object_hash();
                        let tx_not_found = !schema.transactions().contains(&tx_id);
                        let tx_in_pool = schema.transactions_pool().contains(&tx_id);
                        assert!(
                            tx_not_found || tx_in_pool,
                            "Transaction is already committed: {:?}",
                            tx
                        );
                        if tx_not_found {
                            schema.add_transaction_into_pool(tx.clone());
                        }
                        tx_id
                    })
                    .collect()
            };
            self.blockchain.merge(fork.into_patch()).unwrap();
            hashes
        };

        self.create_block_with_tx_hashes(&tx_hashes)
    }

    /// Creates a block with the given transaction.
    /// Transactions that are in the pool will be ignored.
    ///
    /// # Return value
    ///
    /// Returns information about the created block.
    ///
    /// # Panics
    ///
    /// - Panics if given transaction has been already committed to the blockchain.
    pub fn create_block_with_transaction(&mut self, tx: Verified<AnyTx>) -> BlockWithTransactions {
        self.create_block_with_transactions(txvec![tx])
    }

    /// Creates block with the specified transactions. The transactions must be previously
    /// sent to the node via API or directly put into the `channel()`.
    ///
    /// # Return value
    ///
    /// Returns information about the created block.
    ///
    /// # Panics
    ///
    /// - Panics in the case any of transaction hashes are not in the pool.
    pub fn create_block_with_tx_hashes(
        &mut self,
        tx_hashes: &[crypto::Hash],
    ) -> BlockWithTransactions {
        self.poll_events();

        {
            let snapshot = self.blockchain.snapshot();
            let schema = CoreSchema::new(&snapshot);
            for hash in tx_hashes {
                assert!(schema.transactions_pool().contains(hash));
            }
        }

        self.do_create_block(tx_hashes)
    }

    /// Creates block with all transactions in the pool.
    ///
    /// # Return value
    ///
    /// Returns information about the created block.
    pub fn create_block(&mut self) -> BlockWithTransactions {
        self.poll_events();

        let snapshot = self.blockchain.snapshot();
        let schema = CoreSchema::new(&snapshot);
        let txs = schema.transactions_pool();
        let tx_hashes: Vec<_> = txs.iter().collect();
        self.do_create_block(&tx_hashes)
    }

    /// Adds transaction into persistent pool.
    pub fn add_tx(&mut self, transaction: Verified<AnyTx>) {
        let fork = self.blockchain.fork();
        {
            let mut schema = CoreSchema::new(&fork);
            schema.add_transaction_into_pool(transaction);
        }
        self.blockchain
            .merge(fork.into_patch())
            .expect("cannot update database");
    }

    /// Checks if transaction can be found in pool
    pub fn is_tx_in_pool(&self, tx_hash: &Hash) -> bool {
        let snapshot = self.blockchain.snapshot();
        let schema = CoreSchema::new(&snapshot);
        schema.transactions_pool().contains(tx_hash)
    }

    /// Creates a chain of blocks until a given height.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate exonum_testkit;
    /// # extern crate exonum;
    /// # use exonum::helpers::Height;
    /// # use exonum_testkit::TestKitBuilder;
    /// # fn main() {
    /// let mut testkit = TestKitBuilder::validator().create();
    /// testkit.create_blocks_until(Height(5));
    /// assert_eq!(Height(5), testkit.height());
    /// # }
    pub fn create_blocks_until(&mut self, height: Height) {
        while self.height() < height {
            self.create_block();
        }
    }

    /// Returns the hash of latest committed block.
    pub fn last_block_hash(&self) -> crypto::Hash {
        self.blockchain.last_hash()
    }

    /// Returns the height of latest committed block.
    pub fn height(&self) -> Height {
        self.blockchain.last_block().height()
    }

    /// Return an actual blockchain configuration.
    pub fn consensus_config(&self) -> ConsensusConfig {
        CoreSchema::new(&self.snapshot()).consensus_config()
    }

    /// Returns reference to validator with the given identifier.
    ///
    /// # Panics
    ///
    /// - Panics if validator with the given id is absent in test network.
    pub fn validator(&self, id: ValidatorId) -> TestNode {
        self.network.validators()[id.0 as usize].clone()
    }

    /// Returns sufficient number of validators for the Byzantine Fault Tolerance consensus.
    pub fn majority_count(&self) -> usize {
        byzantine_quorum(self.network().validators().len())
    }

    /// Returns the leader on the current height. At the moment first validator.
    pub fn leader(&self) -> TestNode {
        self.network().validators()[0].clone()
    }

    /// Returns the reference to test network.
    pub fn network(&self) -> &TestNetwork {
        &self.network
    }

    /// Returns the mutable reference to test network for manual modifications.
    pub fn network_mut(&mut self) -> &mut TestNetwork {
        &mut self.network
    }

    fn run(mut self, public_api_address: SocketAddr, private_api_address: SocketAddr) {
        let events_stream = self.remove_events_stream();
        // Creates complete actix web server with the testkit extensions.
        let testkit_ref = Arc::new(RwLock::new(self));
        let system_runtime_config = SystemRuntimeConfig {
            api_runtimes: vec![
                ApiRuntimeConfig::new(public_api_address, ApiAccess::Public),
                ApiRuntimeConfig::new(private_api_address, ApiAccess::Private),
            ],
            api_aggregator: server::create_testkit_api_aggregator(&testkit_ref),
        };
        let system_runtime = system_runtime_config.start().unwrap();
        // Run the event stream in a separate thread in order to put transactions to mempool
        // when they are received. Otherwise, a client would need to call a `poll_events` analogue
        // each time after a transaction is posted.
        let mut core = Core::new().unwrap();
        core.run(events_stream).unwrap();

        system_runtime.stop().unwrap();
    }

    /// Extracts the event stream from this testkit, replacing it with `futures::stream::empty()`.
    /// This makes the testkit after the replacement pretty much unusable unless
    /// the old event stream (which is still capable of processing current and future events)
    /// is employed to run to completion.
    ///
    /// # Returned value
    ///
    /// Future that runs the event stream of this testkit to completion.
    fn remove_events_stream(&mut self) -> Box<dyn Future<Item = (), Error = ()>> {
        let stream = std::mem::replace(&mut self.events_stream, Box::new(futures::stream::empty()));
        Box::new(stream.for_each(|_| Ok(())))
    }

    /// Returns the node in the emulated network, from whose perspective the testkit operates.
    pub fn us(&self) -> TestNode {
        self.network().us().clone()
    }

    /// Emulates stopping the node. The stopped node can then be `restart()`ed.
    ///
    /// See [`StoppedTestKit`] documentation for more details how to use this method.
    ///
    /// [`StoppedTestKit`]: struct.StoppedTestKit.html
    pub fn stop(self) -> StoppedTestKit {
        let Self {
            db_handler,
            network,
            ..
        } = self;

        let db = db_handler.into_inner();
        StoppedTestKit { network, db }
    }
}

/// Persistent state of an Exonum node allowing to emulate node restart.
///
/// The persistent state holds the database (including uncommitted transactions) and
/// the network configuration, but does not retain the internal state of the services.
///
/// This method is useful to test scenarios that may play a different way depending
/// on node restarts, such as services with dynamic internal state modified in response
/// to blockchain events (e.g., in `Service::after_commit`).
///
/// # Examples
///
/// ```
/// # use exonum_derive::{exonum_service, ServiceFactory};
/// # use exonum::{
/// #     crypto::{PublicKey, Hash},
/// #     runtime::{InstanceDescriptor, rust::{AfterCommitContext, RustRuntime, Service}},
/// #     helpers::Height,
/// # };
/// # use exonum_merkledb::{Fork, Snapshot};
/// # use exonum_testkit::{StoppedTestKit, TestKit};
/// # use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
/// # const SERVICE_ID: u32 = 1;
/// // Service with internal state modified by a custom `after_commit` hook.
/// # #[derive(Clone, Default, Debug, ServiceFactory)]
/// # #[exonum(
/// #     artifact_name = "after_commit",
/// #     artifact_version = "1.0.0",
/// #     proto_sources = "exonum_testkit::proto",
/// #     service_constructor = "Self::new_instance",
/// #     implements("AfterCommitInterface")
/// # )]
/// struct AfterCommitService {
///     counter: Arc<AtomicUsize>,
/// }
///
/// impl AfterCommitService {
///     pub fn new() -> Self {
///         AfterCommitService { counter: Arc::new(AtomicUsize::default()) }
///     }
///
/// #    pub fn new_instance(&self) -> Box<dyn Service> {
/// #       Box::new(self.clone())
/// #    }
/// #
///     pub fn counter(&self) -> usize {
///         self.counter.load(Ordering::SeqCst)
///     }
/// }
///
/// # #[exonum_service]
/// # trait AfterCommitInterface {}
/// #
/// # impl AfterCommitInterface for AfterCommitService {}
/// #
/// impl Service for AfterCommitService {
/// #   fn state_hash(&self, _: InstanceDescriptor, _: &dyn Snapshot) -> Vec<Hash> { vec![] }
///     fn after_commit(&self, _: AfterCommitContext) {
///         self.counter.fetch_add(1, Ordering::SeqCst);
///     }
/// }
///
/// let service = AfterCommitService::new();
/// let mut testkit = TestKit::for_rust_service(service.clone(), "after_commit", SERVICE_ID, ());
/// testkit.create_blocks_until(Height(5));
/// assert_eq!(service.counter(), 5);
///
/// // Stop the testkit.
/// let stopped = testkit.stop();
/// assert_eq!(stopped.height(), Height(5));
///
/// // Resume with the same single service with a fresh state.
/// let service = AfterCommitService::new();
/// let mut testkit = stopped.resume(vec![
///     RustRuntime::new().with_available_service(service.clone())
/// ]);
/// testkit.create_blocks_until(Height(8));
/// assert_eq!(service.counter(), 3); // We've only created 3 new blocks.
/// ```
#[derive(Debug)]
pub struct StoppedTestKit {
    db: CheckpointDb<TemporaryDB>,
    network: TestNetwork,
}

impl StoppedTestKit {
    /// Return a snapshot of the database state.
    pub fn snapshot(&self) -> Box<dyn Snapshot> {
        self.db.snapshot()
    }

    /// Return the height of latest committed block.
    pub fn height(&self) -> Height {
        let snapshot = self.snapshot();
        CoreSchema::new(&snapshot).height()
    }

    /// Return the reference to test network.
    pub fn network(&self) -> &TestNetwork {
        &self.network
    }

    /// Resume the operation of the testkit.
    ///
    /// Note that `runtimes` may differ from the initially passed to the `TestKit`
    /// (which is also what may happen with real Exonum apps).
    ///
    /// This method will not add the default Rust runtime, so you must do this explicitly.
    pub fn resume(
        self,
        runtimes: impl IntoIterator<Item = impl Into<(u32, Box<dyn Runtime>)>>,
    ) -> TestKit {
        TestKit::assemble(
            self.db,
            self.network,
            // TODO make consensus config optional [ECR-3222]
            ConsensusConfig::default(),
            runtimes.into_iter().map(|x| x.into()),
            // In this context, it is not possible to add new service instances.
            Vec::new(),
        )
    }
}

#[test]
fn test_create_block_heights() {
    let mut testkit = TestKitBuilder::validator().create();
    assert_eq!(Height(0), testkit.height());
    testkit.create_block();
    assert_eq!(Height(1), testkit.height());
    testkit.create_blocks_until(Height(6));
    assert_eq!(Height(6), testkit.height());
}

#[test]
fn test_number_of_validators_in_builder() {
    let testkit = TestKitBuilder::auditor().create();
    assert_eq!(testkit.network().validators().len(), 1);
    assert_ne!(testkit.validator(ValidatorId(0)), testkit.us());

    let testkit = TestKitBuilder::validator().create();
    assert_eq!(testkit.network().validators().len(), 1);
    assert_eq!(testkit.validator(ValidatorId(0)), testkit.us());

    let testkit = TestKitBuilder::auditor().with_validators(3).create();
    assert_eq!(testkit.network().validators().len(), 3);
    let us = testkit.us();
    assert!(!testkit.network().validators().into_iter().any(|v| v == us));

    let testkit = TestKitBuilder::validator().with_validators(5).create();
    assert_eq!(testkit.network().validators().len(), 5);
    assert_eq!(testkit.validator(ValidatorId(0)), testkit.us());
}

#[test]
#[should_panic(expected = "validator should be present")]
fn test_zero_validators_in_builder() {
    let testkit = TestKitBuilder::auditor().with_validators(0).create();
    drop(testkit);
}

#[test]
#[should_panic(expected = "Number of validators is already specified")]
fn test_multiple_spec_of_validators_in_builder() {
    let testkit = TestKitBuilder::auditor()
        .with_validators(5)
        .with_validators(2)
        .create();
    drop(testkit);
}

#[test]
fn test_stop() {
    let testkit = TestKitBuilder::validator().with_logger().create();
    let _testkit_stopped = testkit.stop();
}
