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

//! Testkit for Exonum blockchain framework, allowing to test service APIs synchronously
//! and in the same process as the testkit.
//!
//! # Example
//!
//! ```
//! use exonum::{
//!     blockchain::{Block, Schema},
//!     crypto::{Hash, KeyPair},
//!     helpers::Height,
//!     runtime::{BlockchainData, SnapshotExt, ExecutionError},
//! };
//! use serde_derive::*;
//! use exonum_derive::*;
//! use exonum_merkledb::{ObjectHash, Snapshot};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//! use exonum_rust_runtime::{ServiceFactory, ExecutionContext, Service};
//!
//! // Simple service implementation.
//!
//! const SERVICE_ID: u32 = 1;
//!
//! #[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
//! #[service_dispatcher(implements("TimestampingInterface"))]
//! #[service_factory(
//!     artifact_name = "timestamping",
//!     artifact_version = "1.0.0",
//! )]
//! struct TimestampingService;
//!
//! impl Service for TimestampingService {}
//!
//! #[exonum_interface]
//! pub trait TimestampingInterface<Ctx> {
//!     type Output;
//!     #[interface_method(id = 0)]
//!     fn timestamp(&self, _: Ctx, arg: String) -> Self::Output;
//! }
//!
//! impl TimestampingInterface<ExecutionContext<'_>> for TimestampingService {
//!     type Output = Result<(), ExecutionError>;
//!
//!     fn timestamp(&self, _: ExecutionContext<'_>, arg: String) -> Self::Output {
//!         Ok(())
//!     }
//! }
//!
//! # fn main() {
//! // Create testkit for network with four validators
//! // and add a builtin timestamping service with ID=1.
//! let service = TimestampingService;
//! let artifact = service.artifact_id();
//! let mut testkit = TestKitBuilder::validator()
//!     .with_validators(4)
//!     .with_artifact(artifact.clone())
//!     .with_instance(artifact.into_default_instance(SERVICE_ID, "timestamping"))
//!     .with_rust_service(service)
//!     .build();
//!
//! // Create a few transactions.
//! let keys = KeyPair::random();
//! let id = SERVICE_ID;
//! let tx1 = keys.timestamp(id, "Down To Earth".into());
//! let tx2 = keys.timestamp(id, "Cry Over Spilt Milk".into());
//! let tx3 = keys.timestamp(id, "Dropping Like Flies".into());
//! // Commit them into blockchain.
//! testkit.create_block_with_transactions(vec![
//!     tx1.clone(), tx2.clone(), tx3.clone()
//! ]);
//!
//! // Add a single transaction.
//! let tx4 = keys.timestamp(id, "Barking up the wrong tree".into());
//! testkit.create_block_with_transaction(tx4.clone());
//!
//! // Check results with schema.
//! let snapshot = testkit.snapshot();
//! let schema = snapshot.for_core();
//! assert!(schema.transactions().contains(&tx1.object_hash()));
//! assert!(schema.transactions().contains(&tx2.object_hash()));
//! assert!(schema.transactions().contains(&tx3.object_hash()));
//! assert!(schema.transactions().contains(&tx4.object_hash()));
//! # }
//! ```

#![warn(missing_debug_implementations, missing_docs)]
#![deny(unsafe_code, bare_trait_objects)]

pub use crate::{
    api::{ApiKind, RequestBuilder, TestKitApi},
    builder::TestKitBuilder,
    network::{TestNetwork, TestNode},
};
pub use exonum_explorer as explorer;

use exonum::{
    blockchain::{
        config::GenesisConfig, ApiSender, Blockchain, BlockchainBuilder, BlockchainMut,
        ConsensusConfig,
    },
    crypto::{self, Hash},
    helpers::{byzantine_quorum, Height, ValidatorId},
    merkledb::{BinaryValue, Database, ObjectHash, Snapshot, TemporaryDB},
    messages::{AnyTx, Verified},
    runtime::{InstanceId, RuntimeInstance, SnapshotExt},
};
use exonum_api::{
    backends::actix::SystemRuntime, ApiAccess, ApiAggregator, ApiManager, ApiManagerConfig,
    UpdateEndpoints, WebServerConfig,
};
use exonum_explorer::{BlockWithTransactions, BlockchainExplorer};
use exonum_rust_runtime::{RustRuntimeBuilder, ServiceFactory};
use futures::{sync::mpsc, Future, Stream};
use tokio_core::reactor::Core;

#[cfg(feature = "exonum-node")]
use exonum_node::{ExternalMessage, NodePlugin, PluginApiContext, SharedNodeState};

use std::{
    collections::{BTreeMap, HashMap},
    fmt, iter, mem,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use crate::{
    checkpoint_db::{CheckpointDb, CheckpointDbHandler},
    poll_events::{poll_events, poll_latest},
    server::TestKitActor,
};

mod api;
mod builder;
mod checkpoint_db;
pub mod migrations;
mod network;
mod poll_events;
pub mod server;

type ApiNotifierChannel = (
    mpsc::Sender<UpdateEndpoints>,
    mpsc::Receiver<UpdateEndpoints>,
);

/// Testkit for testing blockchain services. It offers simple network configuration emulation
/// (with no real network setup).
pub struct TestKit {
    blockchain: BlockchainMut,
    db_handler: CheckpointDbHandler<TemporaryDB>,
    events_stream: Box<dyn Stream<Item = (), Error = ()> + Send + Sync>,
    processing_lock: Arc<Mutex<()>>,
    network: TestNetwork,
    api_sender: ApiSender,
    api_notifier_channel: ApiNotifierChannel,
    api_aggregator: ApiAggregator,
    #[cfg(feature = "exonum-node")]
    plugins: Vec<Box<dyn NodePlugin>>,
    #[cfg(feature = "exonum-node")]
    control_channel: (
        mpsc::Sender<ExternalMessage>,
        mpsc::Receiver<ExternalMessage>,
    ),
}

impl fmt::Debug for TestKit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("TestKit")
            .field("blockchain", &self.blockchain)
            .field("network", &self.network)
            .finish()
    }
}

impl TestKit {
    /// Creates a new `TestKit` with a single validator with the given Rust service.
    pub fn for_rust_service(
        service_factory: impl ServiceFactory,
        name: impl Into<String>,
        id: InstanceId,
        constructor: impl BinaryValue,
    ) -> Self {
        let artifact = service_factory.artifact_id();
        TestKitBuilder::validator()
            .with_artifact(artifact.clone())
            .with_instance(
                artifact
                    .into_default_instance(id, name)
                    .with_constructor(constructor),
            )
            .with_rust_service(service_factory)
            .build()
    }

    fn assemble(
        database: impl Into<CheckpointDb<TemporaryDB>>,
        network: TestNetwork,
        genesis_config: Option<GenesisConfig>,
        runtimes: Vec<RuntimeInstance>,
        api_notifier_channel: ApiNotifierChannel,
    ) -> Self {
        let api_channel = mpsc::channel(1_000);
        let api_sender = ApiSender::new(api_channel.0.clone());
        let db = database.into();
        let db_handler = db.handler();
        let db = Arc::new(db);
        let blockchain = Blockchain::new(
            Arc::clone(&db) as Arc<dyn Database>,
            network.us().service_keypair(),
            api_sender.clone(),
        );

        let mut builder = BlockchainBuilder::new(blockchain);
        if let Some(genesis_config) = genesis_config {
            builder = builder.with_genesis_config(genesis_config);
        }
        for runtime in runtimes {
            builder = builder.with_runtime(runtime);
        }
        let blockchain = builder.build();

        let processing_lock = Arc::new(Mutex::new(()));
        let processing_lock_ = Arc::clone(&processing_lock);

        let events_stream: Box<dyn Stream<Item = (), Error = ()> + Send + Sync> =
            Box::new(api_channel.1.and_then(move |transaction| {
                let _guard = processing_lock_.lock().unwrap();
                BlockchainMut::add_transactions_into_db_pool(db.as_ref(), iter::once(transaction));
                Ok(())
            }));

        Self {
            blockchain,
            db_handler,
            api_sender,
            events_stream,
            processing_lock,
            network,
            api_notifier_channel,
            api_aggregator: ApiAggregator::new(),
            #[cfg(feature = "exonum-node")]
            plugins: vec![],
            #[cfg(feature = "exonum-node")]
            control_channel: mpsc::channel(100),
        }
    }

    /// Needs to be called immediately after node creation.
    #[cfg(feature = "exonum-node")]
    pub(crate) fn set_plugins(&mut self, plugins: Vec<Box<dyn NodePlugin>>) {
        debug_assert!(self.plugins.is_empty());
        self.plugins = plugins;
        self.api_aggregator = self.create_api_aggregator();
    }

    #[cfg(feature = "exonum-node")]
    fn create_api_aggregator(&self) -> ApiAggregator {
        let mut aggregator = ApiAggregator::new();
        let node_state = SharedNodeState::new(10_000);
        let plugin_api_context = PluginApiContext::new(
            self.blockchain.as_ref(),
            &node_state,
            ApiSender::new(self.control_channel.0.clone()),
        );
        for plugin in &self.plugins {
            aggregator.extend(plugin.wire_api(plugin_api_context.clone()));
        }
        aggregator
    }

    #[cfg(not(feature = "exonum-node"))]
    fn create_api_aggregator(&self) -> ApiAggregator {
        ApiAggregator::new()
    }

    /// Returns control messages received by the testkit since the last call to this method.
    ///
    /// This method is only available if the crate is compiled with the `exonum-node` feature,
    /// which is off by default.
    #[cfg(feature = "exonum-node")]
    pub fn poll_control_messages(&mut self) -> Vec<ExternalMessage> {
        use crate::poll_events::poll_all;
        poll_all(&mut self.control_channel.1)
    }

    /// Creates an instance of `TestKitApi` to test the API provided by services.
    pub fn api(&mut self) -> TestKitApi {
        TestKitApi::new(self)
    }

    /// Updates API aggregator for the testkit and caches it for further use.
    fn update_aggregator(&mut self) -> ApiAggregator {
        if let Some(Ok(update)) = poll_latest(&mut self.api_notifier_channel.1) {
            let mut aggregator = self.create_api_aggregator();
            aggregator.extend(update.endpoints);
            self.api_aggregator = aggregator;
        }
        self.api_aggregator.clone()
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
        self.blockchain.as_ref().to_owned()
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
    /// # use exonum_derive::{exonum_interface, interface_method, ServiceFactory, ServiceDispatcher, BinaryValue};
    /// # use exonum_testkit::{TestKit, TestKitBuilder};
    /// # use exonum_merkledb::Snapshot;
    /// # use exonum::{crypto::{Hash, KeyPair, PublicKey, SecretKey}, runtime::ExecutionError};
    /// # use exonum_rust_runtime::{ExecutionContext, Service, ServiceFactory};
    /// #
    /// // Suppose we test this service interface:
    /// #[exonum_interface]
    /// pub trait ExampleInterface<Ctx> {
    ///     type Output;
    ///     #[interface_method(id = 0)]
    ///     fn example_tx(&self, ctx: Ctx, arg: String) -> Self::Output;
    /// }
    ///
    /// // ...implemented by this service:
    /// # #[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
    /// # #[service_factory(
    /// #     artifact_name = "example",
    /// #     artifact_version = "1.0.0",
    /// # )]
    /// #[service_dispatcher(implements("ExampleInterface"))]
    /// pub struct ExampleService;
    /// impl Service for ExampleService {}
    /// #
    /// # impl ExampleInterface<ExecutionContext<'_>> for ExampleService {
    /// #     type Output = Result<(), ExecutionError>;
    /// #     fn example_tx(&self, _: ExecutionContext<'_>, _: String) -> Self::Output {
    /// #         Ok(())
    /// #     }
    /// # }
    /// #
    /// # fn expensive_setup(_: &mut TestKit) {}
    /// # fn assert_something_about(_: &TestKit) {}
    /// #
    /// # fn main() {
    /// // ...with this ID:
    /// const SERVICE_ID: u32 = 1;
    ///
    /// let service = ExampleService;
    /// let artifact = service.artifact_id();
    /// let mut testkit = TestKitBuilder::validator()
    ///     .with_artifact(artifact.clone())
    ///     .with_instance(artifact.into_default_instance(SERVICE_ID, "example"))
    ///     .with_rust_service(ExampleService)
    ///     .build();
    /// expensive_setup(&mut testkit);
    /// let keys = KeyPair::random();
    /// let tx_a = keys.example_tx(SERVICE_ID, "foo".into());
    /// let tx_b = keys.example_tx(SERVICE_ID, "bar".into());
    ///
    /// testkit.checkpoint();
    /// testkit.create_block_with_transactions(vec![tx_a.clone(), tx_b.clone()]);
    /// assert_something_about(&testkit);
    /// testkit.rollback();
    ///
    /// testkit.checkpoint();
    /// testkit.create_block_with_transaction(tx_a);
    /// testkit.create_block_with_transaction(tx_b);
    /// assert_something_about(&testkit);
    /// testkit.rollback();
    /// # }
    /// ```
    pub fn rollback(&mut self) {
        self.db_handler.rollback()
    }

    fn do_create_block(&mut self, tx_hashes: &[Hash]) -> BlockWithTransactions {
        let new_block_height = self.height().next();
        let saved_consensus_config = self.consensus_config();
        let validator_id = self.leader().validator_id().unwrap();

        let guard = self.processing_lock.lock().unwrap();
        let (block_hash, patch) = self.blockchain.create_patch(
            validator_id,
            new_block_height,
            tx_hashes,
            &mut BTreeMap::new(),
        );

        let precommits: Vec<_> = self
            .network()
            .validators()
            .iter()
            .map(|v| v.create_precommit(new_block_height, block_hash))
            .collect();

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

        #[cfg(feature = "exonum-node")]
        for plugin in &self.plugins {
            plugin.after_commit(&snapshot);
        }

        BlockchainExplorer::new(&snapshot)
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
    /// - Panics if any of the transactions is incorrect.
    pub fn create_block_with_transactions<I>(&mut self, txs: I) -> BlockWithTransactions
    where
        I: IntoIterator<Item = Verified<AnyTx>>,
    {
        let snapshot = self.snapshot();
        let schema = snapshot.for_core();
        let mut unknown_transactions = vec![];
        let tx_hashes: Vec<_> = txs
            .into_iter()
            .map(|tx| {
                self.check_tx(&tx);

                let tx_id = tx.object_hash();
                let tx_not_found = !schema.transactions().contains(&tx_id);
                let tx_in_pool = schema.transactions_pool().contains(&tx_id);
                assert!(
                    tx_not_found || tx_in_pool,
                    "Transaction is already committed: {:?}",
                    tx
                );
                if tx_not_found {
                    unknown_transactions.push(tx);
                }
                tx_id
            })
            .collect();
        self.blockchain
            .add_transactions_into_pool(unknown_transactions);
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
    /// - Panics if any of the transactions is incorrect.
    pub fn create_block_with_transaction(&mut self, tx: Verified<AnyTx>) -> BlockWithTransactions {
        self.create_block_with_transactions(vec![tx])
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

        let snapshot = self.blockchain.snapshot();
        let schema = snapshot.for_core();
        for hash in tx_hashes {
            assert!(schema.transactions_pool().contains(hash));
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
        let tx_hashes: Vec<_> = self
            .snapshot()
            .for_core()
            .transactions_pool()
            .iter()
            .collect();
        self.do_create_block(&tx_hashes)
    }

    /// Adds transaction into persistent pool.
    pub fn add_tx(&mut self, transaction: Verified<AnyTx>) {
        self.check_tx(&transaction);

        self.blockchain
            .add_transactions_into_pool(iter::once(transaction));
    }

    /// Calls `Blockchain::check_tx` and panics on an error.
    fn check_tx(&self, transaction: &Verified<AnyTx>) {
        if let Err(error) = Blockchain::check_tx(&self.blockchain.snapshot(), &transaction) {
            panic!("Attempt to add invalid tx in the pool: {}", error);
        }
    }

    /// Checks if transaction can be found in pool
    pub fn is_tx_in_pool(&self, tx_hash: &Hash) -> bool {
        self.snapshot()
            .for_core()
            .transactions_pool()
            .contains(tx_hash)
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
    /// let mut testkit = TestKitBuilder::validator().build();
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
        self.blockchain.as_ref().last_hash()
    }

    /// Returns the height of latest committed block.
    pub fn height(&self) -> Height {
        self.blockchain.as_ref().last_block().height
    }

    /// Returns an actual blockchain configuration.
    pub fn consensus_config(&self) -> ConsensusConfig {
        self.snapshot().for_core().consensus_config()
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

    /// Returns the leader on the current height. At the moment, this is always the first validator.
    pub fn leader(&self) -> TestNode {
        self.network().validators()[0].clone()
    }

    /// Returns the reference to the test network.
    pub fn network(&self) -> &TestNetwork {
        &self.network
    }

    /// Returns the mutable reference to test network for manual modifications.
    pub fn network_mut(&mut self) -> &mut TestNetwork {
        &mut self.network
    }

    fn run(mut self, public_api_address: SocketAddr, private_api_address: SocketAddr) {
        let events_stream = self.remove_events_stream();
        let endpoints_rx = mem::replace(&mut self.api_notifier_channel.1, mpsc::channel(0).1);

        let (api_aggregator, actor_handle) = TestKitActor::spawn(self);
        let mut servers = HashMap::new();
        servers.insert(ApiAccess::Public, WebServerConfig::new(public_api_address));
        servers.insert(
            ApiAccess::Private,
            WebServerConfig::new(private_api_address),
        );
        let api_manager_config = ApiManagerConfig {
            servers,
            api_aggregator,
            server_restart_max_retries: 5,
            server_restart_retry_timeout: 500,
        };
        let api_manager = ApiManager::new(api_manager_config, endpoints_rx);
        let system_runtime = SystemRuntime::start(api_manager).unwrap();

        // Run the event stream in a separate thread in order to put transactions to mempool
        // when they are received. Otherwise, a client would need to call a `poll_events` analogue
        // each time after a transaction is posted.
        let mut core = Core::new().unwrap();
        core.run(events_stream).unwrap();
        system_runtime.stop().unwrap();
        actor_handle.join().unwrap();
    }

    /// Extracts the event stream from this testkit, replacing it with `futures::stream::empty()`.
    /// This makes the testkit after the replacement pretty much unusable unless
    /// the old event stream (which is still capable of processing current and future events)
    /// is employed to run to completion.
    ///
    /// # Returned value
    ///
    /// Future that runs the event stream of this testkit to completion.
    pub(crate) fn remove_events_stream(&mut self) -> impl Future<Item = (), Error = ()> {
        let stream = mem::replace(&mut self.events_stream, Box::new(futures::stream::empty()));
        stream.for_each(|_| Ok(()))
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
        let db = self.db_handler.into_inner();
        let network = self.network;
        let api_notifier_channel = self.api_notifier_channel;
        #[cfg(feature = "exonum-node")]
        let plugins = self.plugins;

        StoppedTestKit {
            network,
            db,
            api_notifier_channel,
            #[cfg(feature = "exonum-node")]
            plugins,
        }
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
/// # use exonum_derive::{exonum_interface, ServiceFactory, ServiceDispatcher};
/// # use exonum::{
/// #     crypto::{PublicKey, Hash},
/// #     helpers::Height,
/// #     runtime::BlockchainData,
/// # };
/// # use exonum_rust_runtime::{AfterCommitContext, RustRuntime, Service};
/// # use exonum_merkledb::{Fork, Snapshot};
/// # use exonum_testkit::{StoppedTestKit, TestKit};
/// # use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
/// # const SERVICE_ID: u32 = 1;
/// // Service with internal state modified by a custom `after_commit` hook.
/// # #[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
/// # #[service_factory(
/// #     artifact_name = "after_commit",
/// #     artifact_version = "1.0.0",
/// #     service_constructor = "Self::new_instance",
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
/// impl Service for AfterCommitService {
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
/// let rust_runtime = RustRuntime::builder().with_factory(service.clone());
/// let mut testkit = stopped.resume(rust_runtime);
/// testkit.create_blocks_until(Height(8));
/// assert_eq!(service.counter(), 3); // We've only created 3 new blocks.
/// ```
pub struct StoppedTestKit {
    db: CheckpointDb<TemporaryDB>,
    #[cfg(feature = "exonum-node")]
    plugins: Vec<Box<dyn NodePlugin>>,
    network: TestNetwork,
    api_notifier_channel: ApiNotifierChannel,
}

impl fmt::Debug for StoppedTestKit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoppedTestKit")
            .field("height", &self.height())
            .field("network", &self.network)
            .finish()
    }
}

impl StoppedTestKit {
    /// Returns a snapshot of the database state.
    pub fn snapshot(&self) -> Box<dyn Snapshot> {
        self.db.snapshot()
    }

    /// Returns the height of latest committed block.
    pub fn height(&self) -> Height {
        self.snapshot().for_core().height()
    }

    /// Returns the reference to test network.
    pub fn network(&self) -> &TestNetwork {
        &self.network
    }

    /// Resumes the operation of the testkit with the Rust runtime.
    ///
    /// Note that services in the Rust runtime may differ from the initially passed to the `TestKit`
    /// (which is also what may happen with real Exonum apps).
    pub fn resume(self, rust_runtime: RustRuntimeBuilder) -> TestKit {
        self.resume_with_runtimes(rust_runtime, Vec::new())
    }

    /// Resumes the operation fo the testkit with the specified runtimes.
    pub fn resume_with_runtimes(
        self,
        rust_runtime: RustRuntimeBuilder,
        external_runtimes: Vec<RuntimeInstance>,
    ) -> TestKit {
        let rust_runtime = rust_runtime.build(self.api_notifier_channel.0.clone());
        let mut runtimes = external_runtimes;
        runtimes.push(rust_runtime.into());
        self.do_resume(runtimes)
    }

    #[cfg(feature = "exonum-node")]
    fn do_resume(self, runtimes: Vec<RuntimeInstance>) -> TestKit {
        let mut testkit = TestKit::assemble(
            self.db,
            self.network,
            None,
            runtimes,
            self.api_notifier_channel,
        );
        testkit.set_plugins(self.plugins);
        testkit
    }

    #[cfg(not(feature = "exonum-node"))]
    fn do_resume(self, runtimes: Vec<RuntimeInstance>) -> TestKit {
        TestKit::assemble(
            self.db,
            self.network,
            None,
            runtimes,
            self.api_notifier_channel,
        )
    }
}

#[test]
fn test_create_block_heights() {
    let mut testkit = TestKitBuilder::validator().build();
    assert_eq!(Height(0), testkit.height());
    testkit.create_block();
    assert_eq!(Height(1), testkit.height());
    testkit.create_blocks_until(Height(6));
    assert_eq!(Height(6), testkit.height());
}

#[test]
fn test_number_of_validators_in_builder() {
    let testkit = TestKitBuilder::auditor().build();
    assert_eq!(testkit.network().validators().len(), 1);
    assert_ne!(testkit.validator(ValidatorId(0)), testkit.us());

    let testkit = TestKitBuilder::validator().build();
    assert_eq!(testkit.network().validators().len(), 1);
    assert_eq!(testkit.validator(ValidatorId(0)), testkit.us());

    let testkit = TestKitBuilder::auditor().with_validators(3).build();
    assert_eq!(testkit.network().validators().len(), 3);
    let us = testkit.us();
    assert!(!testkit.network().validators().into_iter().any(|v| v == us));

    let testkit = TestKitBuilder::validator().with_validators(5).build();
    assert_eq!(testkit.network().validators().len(), 5);
    assert_eq!(testkit.validator(ValidatorId(0)), testkit.us());
}

#[test]
#[should_panic(expected = "validator should be present")]
fn test_zero_validators_in_builder() {
    TestKitBuilder::auditor().with_validators(0).build();
}

#[test]
#[should_panic(expected = "Number of validators is already specified")]
fn test_multiple_spec_of_validators_in_builder() {
    let testkit = TestKitBuilder::auditor()
        .with_validators(5)
        .with_validators(2)
        .build();
    drop(testkit);
}

#[test]
fn test_stop() {
    let testkit = TestKitBuilder::validator().with_logger().build();
    let _testkit_stopped = testkit.stop();
}
