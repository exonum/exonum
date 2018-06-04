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

//! Testkit for Exonum blockchain framework, allowing to test service APIs synchronously
//! and in the same process as the testkit.
//!
//! # Example
//! ```
//! #[macro_use]
//! extern crate exonum;
//! #[macro_use]
//! extern crate exonum_testkit;
//! extern crate serde_json;
//!
//! use exonum::api::public::BlocksRange;
//! use exonum::blockchain::{Block, Schema, Service, Transaction, TransactionSet, ExecutionResult};
//! use exonum::crypto::{gen_keypair, Hash, PublicKey, CryptoHash};
//! use exonum::encoding;
//! use exonum::helpers::Height;
//! use exonum::messages::{Message, RawTransaction};
//! use exonum::storage::{Snapshot, Fork};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! // Simple service implementation.
//!
//! const SERVICE_ID: u16 = 1;
//!
//! transactions! {
//!     TimestampingTransactions {
//!         const SERVICE_ID = SERVICE_ID;
//!
//!         struct TxTimestamp {
//!             from: &PublicKey,
//!             msg: &str,
//!         }
//!     }
//! }
//!
//! struct TimestampingService;
//!
//! impl Transaction for TxTimestamp {
//!     fn verify(&self) -> bool {
//!         self.verify_signature(self.from())
//!     }
//!
//!     fn execute(&self, _fork: &mut Fork) -> ExecutionResult {
//!         Ok(())
//!     }
//! }
//!
//! impl Service for TimestampingService {
//!     fn service_name(&self) -> &str {
//!         "timestamping"
//!     }
//!
//!     fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
//!         Vec::new()
//!     }
//!
//!     fn service_id(&self) -> u16 {
//!         SERVICE_ID
//!     }
//!
//!     fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
//!         let tx = TimestampingTransactions::tx_from_raw(raw)?;
//!         Ok(tx.into())
//!     }
//! }
//!
//! fn main() {
//!     // Create testkit for network with four validators.
//!     let mut testkit = TestKitBuilder::validator()
//!         .with_validators(4)
//!         .with_service(TimestampingService)
//!         .create();
//!
//!     // Create few transactions.
//!     let keypair = gen_keypair();
//!     let tx1 = TxTimestamp::new(&keypair.0, "Down To Earth", &keypair.1);
//!     let tx2 = TxTimestamp::new(&keypair.0, "Cry Over Spilt Milk", &keypair.1);
//!     let tx3 = TxTimestamp::new(&keypair.0, "Dropping Like Flies", &keypair.1);
//!     // Commit them into blockchain.
//!     testkit.create_block_with_transactions(txvec![
//!         tx1.clone(), tx2.clone(), tx3.clone()
//!     ]);
//!
//!     // Add a single transaction.
//!     let tx4 = TxTimestamp::new(&keypair.0, "Barking up the wrong tree", &keypair.1);
//!     testkit.create_block_with_transaction(tx4.clone());
//!
//!     // Check results with schema.
//!     let snapshot = testkit.snapshot();
//!     let schema = Schema::new(&snapshot);
//!     assert!(schema.transactions().contains(&tx1.hash()));
//!     assert!(schema.transactions().contains(&tx2.hash()));
//!     assert!(schema.transactions().contains(&tx3.hash()));
//!     assert!(schema.transactions().contains(&tx4.hash()));
//!
//!     // Check results with api.
//!     let api = testkit.api();
//!     let response: BlocksRange = api.get(ApiKind::Explorer, "v1/blocks?count=10");
//!     let (blocks, range) = (response.blocks, response.range);
//!     assert_eq!(blocks.len(), 3);
//!     assert_eq!(range.start, Height(0));
//!     assert_eq!(range.end, Height(3));
//!
//!     api.get::<serde_json::Value>(
//!         ApiKind::Explorer,
//!         &format!("v1/transactions/{}", tx1.hash().to_string()),
//!     );
//! }
//! ```

#![deny(missing_debug_implementations, missing_docs)]

extern crate bodyparser;
#[cfg_attr(test, macro_use)]
extern crate exonum;
extern crate futures;
extern crate iron;
extern crate iron_test;
#[macro_use]
extern crate log;
extern crate mount;
extern crate router;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[cfg_attr(test, macro_use)]
extern crate serde_json;
extern crate tokio_core;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

pub use api::{ApiKind, TestKitApi};
pub use compare::ComparableSnapshot;
pub use network::{TestNetwork, TestNetworkConfiguration, TestNode};

pub mod compare;

use futures::{sync::mpsc, Future, Stream};
use iron::Iron;
use tokio_core::reactor::Core;

use std::{fmt,
          net::SocketAddr,
          sync::{Arc, RwLock},
          thread};

use exonum::{blockchain::{Blockchain, Schema as CoreSchema, Service, StoredConfiguration,
                          Transaction},
             crypto::{self, Hash},
             explorer::{BlockWithTransactions, BlockchainExplorer},
             helpers::{Height, ValidatorId},
             messages::RawMessage,
             node::{ApiSender, ExternalMessage, NodeApiConfig, State as NodeState},
             storage::{MemoryDB, Patch, Snapshot}};

use checkpoint_db::{CheckpointDb, CheckpointDbHandler};
use handler::create_testkit_handler;
use poll_events::poll_events;

#[macro_use]
mod macros;
mod api;
mod checkpoint_db;
mod handler;
mod network;
mod poll_events;

/// Builder for `TestKit`.
///
/// # Testkit server
///
/// By calling the [`serve`] method, you can transform testkit into a web server useful for
/// client-side testing. The testkit-specific APIs are exposed on the private address
/// with the `/api/testkit` prefix (hereinafter denoted as `{baseURL}`).
/// In all APIs, the request body (if applicable) and response are JSON-encoded.
///
/// ## Testkit status
///
/// GET `{baseURL}/v1/status`
///
/// Outputs the status of the testkit, which includes:
///
/// - Current blockchain height
/// - Current [test network configuration][cfg]
/// - Next network configuration if it is scheduled with [`commit_configuration_change`].
///
/// ## Create block
///
/// POST `{baseURL}/v1/blocks`
///
/// Creates a new block in the testkit blockchain. If the
/// JSON body of the request is an empty object, the call is functionally equivalent
/// to [`create_block`]. Otherwise, if the body has the `tx_hashes` field specifying an array
/// of transaction hashes, the call is equivalent to [`create_block_with_tx_hashes`] supplied
/// with these hashes.
///
/// Returns the latest block from the blockchain on success.
///
/// ## Roll back
///
/// DELETE `{baseURL}/v1/blocks/:height`
///
/// Acts as a rough [`rollback`] equivalent. The blocks are rolled back up and including the block
/// at the specified `height` (a positive integer), so that after the request the blockchain height
/// is equal to `height - 1`. If the specified height is greater than the blockchain height,
/// the request performs no action.
///
/// Returns the latest block from the blockchain on success.
///
/// [`serve`]: #method.serve
/// [cfg]: struct.TestNetworkConfiguration.html
/// [`create_block`]: struct.TestKit.html#method.create_block
/// [`create_block_with_tx_hashes`]: struct.TestKit.html#method.create_block_with_tx_hashes
/// [`commit_configuration_change`]: struct.TestKit.html#method.commit_configuration_change
/// [`rollback`]: struct.TestKit.html#method.rollback
///
/// # Example
///
/// ```
/// # extern crate exonum;
/// # extern crate exonum_testkit;
/// # use exonum::blockchain::{Service, Transaction};
/// # use exonum::messages::RawTransaction;
/// # use exonum::encoding;
/// # use exonum_testkit::TestKitBuilder;
/// # pub struct MyService;
/// # impl Service for MyService {
/// #    fn service_name(&self) -> &str {
/// #        "documentation"
/// #    }
/// #    fn state_hash(&self, _: &exonum::storage::Snapshot) -> Vec<exonum::crypto::Hash> {
/// #        Vec::new()
/// #    }
/// #    fn service_id(&self) -> u16 {
/// #        0
/// #    }
/// #    fn tx_from_raw(&self, _raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
/// #        unimplemented!();
/// #    }
/// # }
/// # fn main() {
/// let mut testkit = TestKitBuilder::validator()
///     .with_service(MyService)
///     .with_validators(4)
///     .create();
/// testkit.create_block();
/// // Other test code
/// # }
/// ```
pub struct TestKitBuilder {
    our_validator_id: Option<ValidatorId>,
    validator_count: Option<u16>,
    services: Vec<Box<Service>>,
    logger: bool,
}

impl fmt::Debug for TestKitBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TestKitBuilder")
            .field(
                "us",
                &self.our_validator_id
                    .map_or("Auditor".to_string(), |id| format!("Validator #{}", id.0)),
            )
            .field("validator_count", &self.validator_count)
            .field(
                "services",
                &self.services
                    .iter()
                    .map(|x| x.service_name())
                    .collect::<Vec<_>>(),
            )
            .field("logger", &self.logger)
            .finish()
    }
}

impl TestKitBuilder {
    /// Creates testkit for the validator node.
    pub fn validator() -> Self {
        TestKitBuilder {
            validator_count: None,
            our_validator_id: Some(ValidatorId(0)),
            services: Vec::new(),
            logger: false,
        }
    }

    /// Creates testkit for the auditor node.
    pub fn auditor() -> Self {
        TestKitBuilder {
            validator_count: None,
            our_validator_id: None,
            services: Vec::new(),
            logger: false,
        }
    }

    /// Sets the number of validator nodes in the test network.
    pub fn with_validators(mut self, validator_count: u16) -> Self {
        assert!(
            self.validator_count.is_none(),
            "Number of validators is already specified"
        );
        self.validator_count = Some(validator_count);
        self
    }

    /// Adds a service to the testkit.
    pub fn with_service<S>(mut self, service: S) -> Self
    where
        S: Into<Box<Service>>,
    {
        self.services.push(service.into());
        self
    }

    /// Enables a logger inside the testkit.
    pub fn with_logger(mut self) -> Self {
        self.logger = true;
        self
    }

    /// Creates the testkit.
    pub fn create(self) -> TestKit {
        if self.logger {
            exonum::helpers::init_logger().ok();
        }
        crypto::init();
        TestKit::assemble(
            self.services,
            TestNetwork::with_our_role(self.our_validator_id, self.validator_count.unwrap_or(1)),
        )
    }

    /// Starts a testkit web server, which listens to public and private APIs exposed by
    /// the testkit, on the respective addresses. The private address also exposes the testkit
    /// APIs with the `/api/testkit` URL prefix.
    ///
    /// Unlike real Exonum nodes, the testkit web server does not create peer-to-peer connections
    /// with other nodes, and does not create blocks automatically. The only way to commit
    /// transactions is thus to use the [testkit API](#testkit-server).
    pub fn serve(self, public_api_address: SocketAddr, private_api_address: SocketAddr) {
        let testkit = self.create();
        testkit.run(public_api_address, private_api_address);
    }
}

/// Testkit for testing blockchain services. It offers simple network configuration emulation
/// (with no real network setup).
pub struct TestKit {
    blockchain: Blockchain,
    db_handler: CheckpointDbHandler<MemoryDB>,
    events_stream: Box<Stream<Item = (), Error = ()> + Send + Sync>,
    network: TestNetwork,
    api_sender: ApiSender,
    cfg_proposal: Option<ConfigurationProposalState>,
    api_config: NodeApiConfig,
}

impl fmt::Debug for TestKit {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TestKit")
            .field("blockchain", &self.blockchain)
            .field("network", &self.network)
            .field("cfg_change_proposal", &self.cfg_proposal)
            .finish()
    }
}

impl TestKit {
    /// Creates a new `TestKit` with a single validator with the given service.
    pub fn for_service<S>(service: S) -> Self
    where
        S: Into<Box<Service>>,
    {
        TestKitBuilder::validator().with_service(service).create()
    }

    fn assemble(services: Vec<Box<Service>>, network: TestNetwork) -> Self {
        let api_channel = mpsc::channel(1_000);
        let api_sender = ApiSender::new(api_channel.0.clone());

        let db = CheckpointDb::new(MemoryDB::new());
        let db_handler = db.handler();

        let mut blockchain = Blockchain::new(
            db,
            services,
            *network.us().service_keypair().0,
            network.us().service_keypair().1.clone(),
            api_sender.clone(),
        );

        let genesis = network.genesis_config();
        blockchain.initialize(genesis.clone()).unwrap();

        let events_stream: Box<Stream<Item = (), Error = ()> + Send + Sync> = {
            let mut blockchain = blockchain.clone();
            Box::new(api_channel.1.and_then(move |event| {
                let mut fork = blockchain.fork();
                {
                    let mut schema = CoreSchema::new(&mut fork);
                    match event {
                        ExternalMessage::Transaction(tx) => {
                            let hash = tx.hash();
                            if !schema.transactions().contains(&hash) {
                                schema.add_transaction_into_pool(tx.raw().clone());
                            }
                        }
                        ExternalMessage::PeerAdd(_)
                        | ExternalMessage::Enable(_)
                        | ExternalMessage::Shutdown => { /* Ignored */ }
                    }
                }
                blockchain.merge(fork.into_patch()).unwrap();
                Ok(())
            }))
        };

        TestKit {
            blockchain,
            db_handler,
            api_sender,
            events_stream,
            network,
            cfg_proposal: None,
            api_config: Default::default(),
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
    pub fn snapshot(&self) -> Box<Snapshot> {
        self.blockchain.snapshot()
    }

    /// Returns a reference to the blockchain used by the testkit.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Returns a blockchain instance for low level manipulations with storage.
    pub fn blockchain_mut(&mut self) -> &mut Blockchain {
        &mut self.blockchain
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
    /// # #[macro_use] extern crate exonum;
    /// # #[macro_use] extern crate exonum_testkit;
    /// # use exonum::blockchain::{Service, Transaction, TransactionSet, ExecutionResult};
    /// # use exonum::messages::RawTransaction;
    /// # use exonum::encoding;
    /// # use exonum_testkit::{TestKit, TestKitBuilder};
    /// #
    /// # type FromRawResult = Result<Box<Transaction>, encoding::Error>;
    /// # pub struct MyService;
    /// # impl Service for MyService {
    /// #    fn service_name(&self) -> &str {
    /// #        "documentation"
    /// #    }
    /// #    fn state_hash(&self, _: &exonum::storage::Snapshot) -> Vec<exonum::crypto::Hash> {
    /// #        Vec::new()
    /// #    }
    /// #    fn service_id(&self) -> u16 {
    /// #        1
    /// #    }
    /// #    fn tx_from_raw(&self, raw: RawTransaction) -> FromRawResult {
    /// #        let tx = MyServiceTransactions::tx_from_raw(raw)?;
    /// #        Ok(tx.into())
    /// #    }
    /// # }
    /// #
    /// # transactions! {
    /// #     MyServiceTransactions {
    /// #         const SERVICE_ID = 1;
    /// #
    /// #         struct MyTransaction {
    /// #             from: &exonum::crypto::PublicKey,
    /// #             msg: &str,
    /// #         }
    /// #     }
    /// # }
    /// # impl Transaction for MyTransaction {
    /// #     fn verify(&self) -> bool { true }
    /// #     fn execute(&self, _: &mut exonum::storage::Fork) -> ExecutionResult { Ok(()) }
    /// # }
    /// #
    /// # fn expensive_setup(_: &mut TestKit) {}
    /// # fn assert_something_about(_: &TestKit) {}
    /// #
    /// # fn main() {
    /// let mut testkit = TestKitBuilder::validator()
    ///     .with_service(MyService)
    ///     .create();
    /// expensive_setup(&mut testkit);
    /// let (pubkey, key) = exonum::crypto::gen_keypair();
    /// let tx_a = MyTransaction::new(&pubkey, "foo", &key);
    /// let tx_b = MyTransaction::new(&pubkey, "bar", &key);
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
    pub fn probe_all<I>(&mut self, transactions: I) -> Box<Snapshot>
    where
        I: IntoIterator<Item = Box<Transaction>>,
    {
        self.poll_events();
        // Filter out already committed transactions; otherwise,
        // `create_block_with_transactions()` will panic.
        let schema = CoreSchema::new(self.snapshot());
        let uncommitted_txs = transactions.into_iter().filter(|tx| {
            !schema.transactions().contains(&tx.hash())
                || schema.transactions_pool().contains(&tx.hash())
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
    pub fn probe<T: Transaction>(&mut self, transaction: T) -> Box<Snapshot> {
        self.probe_all(vec![Box::new(transaction) as Box<Transaction>])
    }

    fn do_create_block(&mut self, tx_hashes: &[crypto::Hash]) -> BlockWithTransactions {
        let new_block_height = self.height().next();
        let last_hash = self.last_block_hash();

        let config_patch = self.update_configuration(new_block_height);
        let (block_hash, patch) = {
            let validator_id = self.leader().validator_id().unwrap();
            self.blockchain
                .create_patch(validator_id, new_block_height, tx_hashes)
        };

        let patch = if let Some(config_patch) = config_patch {
            let mut fork = self.blockchain.fork();
            fork.merge(config_patch);
            fork.merge(patch);
            fork.into_patch()
        } else {
            patch
        };

        let propose = self.leader()
            .create_propose(new_block_height, &last_hash, tx_hashes);
        let precommits: Vec<_> = self.network()
            .validators()
            .iter()
            .map(|v| v.create_precommit(&propose, &block_hash))
            .collect();

        self.blockchain
            .commit(&patch, block_hash, precommits.iter())
            .unwrap();

        self.poll_events();

        BlockchainExplorer::new(&self.blockchain)
            .block_with_txs(self.height())
            .unwrap()
    }

    /// Update test network configuration if such an update has been scheduled
    /// with `commit_configuration_change`.
    fn update_configuration(&mut self, new_block_height: Height) -> Option<Patch> {
        use ConfigurationProposalState::*;

        let actual_from = new_block_height.next();
        if let Some(cfg_proposal) = self.cfg_proposal.take() {
            match cfg_proposal {
                Uncommitted(cfg_proposal) => {
                    // Commit configuration proposal
                    let stored = cfg_proposal.stored_configuration().clone();

                    let mut fork = self.blockchain.fork();
                    CoreSchema::new(&mut fork).commit_configuration(stored);
                    self.cfg_proposal = Some(Committed(cfg_proposal));

                    return Some(fork.into_patch());
                }
                Committed(cfg_proposal) => {
                    if cfg_proposal.actual_from() == actual_from {
                        // Modify the self configuration
                        self.network_mut().update_configuration(cfg_proposal);
                    } else {
                        self.cfg_proposal = Some(Committed(cfg_proposal));
                    }
                }
            }
        }

        None
    }

    /// Returns a reference to the scheduled configuration proposal, or `None` if
    /// there is no such proposal.
    pub fn next_configuration(&self) -> Option<&TestNetworkConfiguration> {
        use ConfigurationProposalState::*;

        self.cfg_proposal.as_ref().map(|p| match *p {
            Committed(ref proposal) | Uncommitted(ref proposal) => proposal,
        })
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
        I: IntoIterator<Item = Box<Transaction>>,
    {
        let tx_hashes: Vec<_> = {
            let blockchain = self.blockchain_mut();
            let mut fork = blockchain.fork();
            let hashes = {
                let mut schema = CoreSchema::new(&mut fork);

                txs.into_iter()
                    .filter(|tx| tx.verify())
                    .map(|tx| {
                        let tx_id = tx.hash();
                        let tx_not_found = !schema.transactions().contains(&tx_id);
                        let tx_in_pool = schema.transactions_pool().contains(&tx_id);
                        assert!(
                            tx_not_found || tx_in_pool,
                            "Transaction is already committed: {:?}",
                            tx
                        );
                        schema.add_transaction_into_pool(tx.raw().clone());

                        tx_id
                    })
                    .collect()
            };
            blockchain.merge(fork.into_patch()).unwrap();
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
    pub fn create_block_with_transaction<T: Transaction>(
        &mut self,
        tx: T,
    ) -> BlockWithTransactions {
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
        //TODO: every block should contain two merges (ECR-975)
        {
            let blockchain = self.blockchain_mut();
            let fork = blockchain.fork();
            blockchain.merge(fork.into_patch()).unwrap();
        }
        self.do_create_block(&tx_hashes)
    }

    /// Adds transaction into persistent pool.
    pub fn add_tx(&mut self, transaction: RawMessage) {
        let mut fork = self.blockchain.fork();
        let mut schema = CoreSchema::new(&mut fork);
        schema.add_transaction_into_pool(transaction)
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

    /// Returns the blockchain explorer instance.
    pub fn explorer(&self) -> BlockchainExplorer {
        BlockchainExplorer::new(&self.blockchain)
    }

    /// Returns the actual blockchain configuration.
    pub fn actual_configuration(&self) -> StoredConfiguration {
        CoreSchema::new(&self.snapshot()).actual_configuration()
    }

    /// Returns reference to validator with the given identifier.
    ///
    /// # Panics
    ///
    /// - Panics if validator with the given id is absent in test network.
    pub fn validator(&self, id: ValidatorId) -> &TestNode {
        &self.network.validators()[id.0 as usize]
    }

    /// Returns sufficient number of validators for the Byzantine Fault Tolerance consensus.
    pub fn majority_count(&self) -> usize {
        NodeState::byzantine_majority_count(self.network().validators().len())
    }

    /// Returns the leader on the current height. At the moment first validator.
    pub fn leader(&self) -> &TestNode {
        &self.network().validators()[0]
    }

    /// Returns the reference to test network.
    pub fn network(&self) -> &TestNetwork {
        &self.network
    }

    /// Returns the mutable reference to test network for manual modifications.
    pub fn network_mut(&mut self) -> &mut TestNetwork {
        &mut self.network
    }

    /// Returns a copy of the actual configuration of the testkit.
    /// The returned configuration could be modified for use with
    /// `commit_configuration_change` method.
    pub fn configuration_change_proposal(&self) -> TestNetworkConfiguration {
        let stored_configuration = CoreSchema::new(&self.snapshot()).actual_configuration();
        TestNetworkConfiguration::new(self.network(), stored_configuration)
    }

    /// Adds a new configuration proposal. Remember, to add this proposal to the blockchain,
    /// you should create at least one block.
    ///
    /// # Panics
    ///
    /// - Panics if `actual_from` is less than current height or equals.
    /// - Panics if configuration change has been already proposed but not executed.
    ///
    /// # Example
    ///
    /// ```
    /// extern crate exonum;
    /// extern crate exonum_testkit;
    /// extern crate serde;
    /// extern crate serde_json;
    ///
    /// use exonum::blockchain::Schema;
    /// use exonum::crypto::CryptoHash;
    /// use exonum::helpers::{Height, ValidatorId};
    /// use exonum_testkit::TestKitBuilder;
    ///
    /// fn main() {
    ///    let mut testkit = TestKitBuilder::auditor().with_validators(3).create();
    ///
    ///    let cfg_change_height = Height(5);
    ///    let proposal = {
    ///         let mut cfg = testkit.configuration_change_proposal();
    ///         // Add us to validators.
    ///         let mut validators = cfg.validators().to_vec();
    ///         validators.push(testkit.network().us().clone());
    ///         cfg.set_validators(validators);
    ///         // Change configuration of our service.
    ///         cfg.set_service_config("my_service", "My config");
    ///         // Set the height with which the configuration takes effect.
    ///         cfg.set_actual_from(cfg_change_height);
    ///         cfg
    ///     };
    ///     // Save proposed configuration.
    ///     let stored = proposal.stored_configuration().clone();
    ///     // Commit configuration change proposal to the testkit.
    ///     testkit.commit_configuration_change(proposal);
    ///     // Create blocks up to the height preceding the `actual_from` height.
    ///     testkit.create_blocks_until(cfg_change_height.previous());
    ///     // Check that the proposal has become actual.
    ///     assert_eq!(testkit.network().us().validator_id(), Some(ValidatorId(3)));
    ///     assert_eq!(testkit.validator(ValidatorId(3)), testkit.network().us());
    ///     assert_eq!(testkit.actual_configuration(), stored);
    ///     assert_eq!(
    ///         Schema::new(&testkit.snapshot())
    ///             .previous_configuration()
    ///             .unwrap()
    ///             .hash(),
    ///         stored.previous_cfg_hash
    ///     );
    /// }
    /// ```
    pub fn commit_configuration_change(&mut self, proposal: TestNetworkConfiguration) {
        use self::ConfigurationProposalState::*;

        assert!(
            self.height() < proposal.actual_from(),
            "The `actual_from` height should be greater than the current."
        );
        assert!(
            self.cfg_proposal.is_none(),
            "There is an active configuration change proposal."
        );
        self.cfg_proposal = Some(Uncommitted(proposal));
    }

    fn run(mut self, public_api_address: SocketAddr, private_api_address: SocketAddr) {
        let api = self.api();
        let events_stream = self.remove_events_stream();
        let testkit_ref = Arc::new(RwLock::new(self));
        let (public_handler, private_handler) =
            api.into_handlers(create_testkit_handler(&testkit_ref));

        let public_api_thread = thread::spawn(move || {
            Iron::new(public_handler).http(public_api_address).unwrap();
        });
        let private_api_thread = thread::spawn(move || {
            Iron::new(private_handler)
                .http(private_api_address)
                .unwrap();
        });

        // Run the event stream in a separate thread in order to put transactions to mempool
        // when they are received. Otherwise, a client would need to call a `poll_events` analogue
        // each time after a transaction is posted.
        let mut core = Core::new().unwrap();
        core.run(events_stream).unwrap();

        public_api_thread.join().unwrap();
        private_api_thread.join().unwrap();
    }

    /// Extracts the event stream from this testkit, replacing it with `futures::stream::empty()`.
    /// This makes the testkit after the replacement pretty much unusable unless
    /// the old event stream (which is still capable of processing current and future events)
    /// is employed to run to completion.
    ///
    /// # Returned value
    ///
    /// Future that runs the event stream of this testkit to completion.
    fn remove_events_stream(&mut self) -> Box<Future<Item = (), Error = ()>> {
        let stream = std::mem::replace(&mut self.events_stream, Box::new(futures::stream::empty()));
        Box::new(stream.for_each(|_| Ok(())))
    }

    /// Returns the node in the emulated network, from whose perspective the testkit operates.
    pub fn us(&self) -> &TestNode {
        self.network().us()
    }
}

// A new configuration proposal state.
#[derive(Debug)]
enum ConfigurationProposalState {
    Uncommitted(TestNetworkConfiguration),
    Committed(TestNetworkConfiguration),
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
    assert!(!testkit.network().validators().iter().any(|v| v == us));

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
