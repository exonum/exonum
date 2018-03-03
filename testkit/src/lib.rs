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
//! use exonum::crypto::{gen_keypair, Hash, PublicKey, CryptoHash};
//! use exonum::blockchain::{Block, Schema, Service, Transaction, TransactionSet, ExecutionResult};
//! use exonum::explorer::BlocksRange;
//! use exonum::messages::{Message, RawTransaction};
//! use exonum::storage::{Snapshot, Fork};
//! use exonum::encoding;
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
//!     assert_eq!(range.start, 0);
//!     assert_eq!(range.end, 3);
//!
//!     api.get::<serde_json::Value>(
//!         ApiKind::Explorer,
//!         &format!("v1/transactions/{}", tx1.hash().to_string()),
//!     );
//! }
//! ```

#![deny(missing_debug_implementations, missing_docs)]

extern crate exonum;
extern crate futures;
extern crate iron;
extern crate iron_test;
extern crate router;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate log;

use futures::Stream;
use futures::executor::{self, Spawn};
use futures::sync::mpsc;

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard};
use std::fmt;

use exonum::blockchain::{Blockchain, Schema as CoreSchema, Service, StoredConfiguration,
                         Transaction};
use exonum::crypto;
use exonum::helpers::{Height, ValidatorId};
use exonum::node::{ApiSender, ExternalMessage, State as NodeState, TxPool, NodeApiConfig};
use exonum::storage::{MemoryDB, Snapshot};

#[macro_use]
mod macros;
mod api;
mod checkpoint_db;
pub mod compare;
mod greedy_fold;
mod network;

pub use api::{ApiKind, TestKitApi};
pub use compare::ComparableSnapshot;
pub use network::{TestNetwork, TestNode, TestNetworkConfiguration};

use checkpoint_db::{CheckpointDb, CheckpointDbHandler};
use greedy_fold::GreedilyFoldable;

/// Builder for `TestKit`.
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
                &self.our_validator_id.map_or("Auditor".to_string(), |id| {
                    format!("Validator #{}", id.0)
                }),
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
            exonum::helpers::init_logger().unwrap();
        }
        crypto::init();
        TestKit::assemble(
            self.services,
            TestNetwork::with_our_role(self.our_validator_id, self.validator_count.unwrap_or(1)),
        )
    }
}

/// Testkit for testing blockchain services. It offers simple network configuration emulation
/// (with no real network setup).
pub struct TestKit {
    blockchain: Blockchain,
    db_handler: CheckpointDbHandler<MemoryDB>,
    events_stream: Spawn<Box<Stream<Item = (), Error = ()>>>,
    network: TestNetwork,
    api_sender: ApiSender,
    mempool: TxPool,
    cfg_proposal: Option<ConfigurationProposalState>,
    api_config: NodeApiConfig,
}

impl fmt::Debug for TestKit {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TestKit")
            .field("blockchain", &self.blockchain)
            .field("network", &self.network)
            .field("mempool", &self.mempool)
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

        let mempool = Arc::new(RwLock::new(BTreeMap::new()));
        let event_stream: Box<Stream<Item = (), Error = ()>> = {
            let blockchain = blockchain.clone();
            let mempool = Arc::clone(&mempool);
            Box::new(api_channel.1.greedy_fold((), move |_, event| {
                let snapshot = blockchain.snapshot();
                let schema = CoreSchema::new(&snapshot);
                match event {
                    ExternalMessage::Transaction(tx) => {
                        let hash = tx.hash();
                        if !schema.transactions().contains(&hash) {
                            mempool
                                .write()
                                .expect("Cannot write transactions to mempool")
                                .insert(tx.hash(), tx);
                        }
                    }
                    ExternalMessage::PeerAdd(_) |
                    ExternalMessage::Enable(_) |
                    ExternalMessage::Shutdown => { /* Ignored */ }
                }
            }))
        };
        let events_stream = executor::spawn(event_stream);

        TestKit {
            blockchain,
            db_handler,
            api_sender,
            events_stream,
            network,
            mempool: Arc::clone(&mempool),
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
    pub fn poll_events(&mut self) -> Option<Result<(), ()>> {
        self.events_stream.wait_stream()
    }

    /// Returns a snapshot of the current blockchain state.
    pub fn snapshot(&self) -> Box<Snapshot> {
        self.blockchain.snapshot()
    }

    /// Returns a blockchain instance for low level manipulations with storage.
    pub fn blockchain_mut(&mut self) -> &mut Blockchain {
        &mut self.blockchain
    }

    /// Rolls the blockchain back for a certain number of blocks.
    ///
    /// # Examples
    ///
    /// Rollbacks are useful in testing alternative scenarios (e.g., transactions executed
    /// in different order and/or in different blocks) that require an expensive setup:
    ///
    /// ```
    /// # #[macro_use] extern crate exonum;
    /// # #[macro_use] extern crate exonum_testkit;
    /// # use exonum::blockchain::{Service, Transaction, ExecutionResult};
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
    /// #        0
    /// #    }
    /// #    fn tx_from_raw(&self, _raw: RawTransaction) -> FromRawResult {
    /// #        unimplemented!();
    /// #    }
    /// # }
    /// #
    /// # transactions! {
    /// #     Transactions {
    /// #         const SERVICE_ID = 0;
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
    /// testkit.create_block_with_transactions(txvec![tx_a.clone(), tx_b.clone()]);
    /// assert_something_about(&testkit);
    /// testkit.rollback(1);
    /// testkit.create_block_with_transactions(txvec![tx_a.clone()]);
    /// testkit.create_block_with_transactions(txvec![tx_b.clone()]);
    /// assert_something_about(&testkit);
    /// testkit.rollback(2);
    /// # }
    /// ```
    pub fn rollback(&mut self, blocks: usize) {
        assert!(
            (blocks as u64) <= self.height().0,
            "Cannot rollback past genesis block"
        );
        self.db_handler.rollback(blocks);
    }

    /// Executes a list of transactions given the current state of the blockchain, but does not
    /// commit execution results to the blockchain. The execution result is the same
    /// as if transactions were included into a new block; for example,
    /// transactions included into one of previous blocks do not lead to any state changes.
    pub fn probe_all<I>(&mut self, transactions: I) -> Box<Snapshot>
    where
        I: IntoIterator<Item = Box<Transaction>>,
    {
        // Filter out already committed transactions; otherwise,
        // `create_block_with_transactions()` will panic.
        let schema = CoreSchema::new(self.snapshot());
        let uncommitted_txs = transactions.into_iter().filter(|tx| {
            !schema.transactions().contains(&tx.hash())
        });

        self.create_block_with_transactions(uncommitted_txs);
        let snapshot = self.snapshot();
        self.rollback(1);
        snapshot
    }

    /// Executes a transaction given the current state of the blockchain but does not
    /// commit execution results to the blockchain. The execution result is the same
    /// as if a transaction was included into a new block; for example,
    /// a transaction included into one of previous blocks does not lead to any state changes.
    pub fn probe<T: Transaction>(&mut self, transaction: T) -> Box<Snapshot> {
        self.probe_all(vec![Box::new(transaction) as Box<Transaction>])
    }

    fn do_create_block(&mut self, tx_hashes: &[crypto::Hash]) {
        let new_block_height = self.height().next();
        let last_hash = self.last_block_hash();

        self.update_configuration(new_block_height);
        let (block_hash, patch) = {
            let validator_id = self.leader().validator_id().unwrap();
            let transactions = self.mempool();
            self.blockchain.create_patch(
                validator_id,
                new_block_height,
                tx_hashes,
                &transactions,
            )
        };

        // Remove txs from mempool
        {
            let mut transactions = self.mempool.write().expect(
                "Cannot modify transactions in mempool",
            );
            for hash in tx_hashes {
                transactions.remove(hash);
            }
        }

        let propose = self.leader().create_propose(
            new_block_height,
            &last_hash,
            tx_hashes,
        );
        let precommits: Vec<_> = self.network()
            .validators()
            .iter()
            .map(|v| v.create_precommit(&propose, &block_hash))
            .collect();

        self.blockchain
            .commit(&patch, block_hash, precommits.iter())
            .unwrap();

        self.poll_events();
    }

    /// Update test network configuration if such an update has been scheduled
    /// with `commit_configuration_change`.
    fn update_configuration(&mut self, new_block_height: Height) {
        use ConfigurationProposalState::*;

        let actual_from = new_block_height.next();
        if let Some(cfg_proposal) = self.cfg_proposal.take() {
            match cfg_proposal {
                Uncommitted(cfg_proposal) => {
                    // Commit configuration proposal
                    let stored = cfg_proposal.stored_configuration().clone();
                    let mut fork = self.blockchain.fork();
                    CoreSchema::new(&mut fork).commit_configuration(stored);
                    let changes = fork.into_patch();
                    self.blockchain.merge(changes).unwrap();
                    self.cfg_proposal = Some(Committed(cfg_proposal));
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
    }

    /// Creates a block with the given transactions.
    /// Transactions that are in the mempool will be ignored.
    ///
    /// # Panics
    ///
    /// - Panics if any of transactions has been already committed to the blockchain.
    pub fn create_block_with_transactions<I>(&mut self, txs: I)
    where
        I: IntoIterator<Item = Box<Transaction>>,
    {
        let tx_hashes: Vec<_> = {
            let mut mempool = self.mempool.write().expect(
                "Cannot write transactions to mempool",
            );

            let snapshot = self.snapshot();
            let schema = CoreSchema::new(&snapshot);
            txs.into_iter()
                .filter(|tx| tx.verify())
                .map(|tx| {
                    let tx_id = tx.hash();
                    assert!(
                        !schema.transactions().contains(&tx_id),
                        "Transaction is already committed: {:?}",
                        tx
                    );
                    mempool.insert(tx_id, tx);
                    tx_id
                })
                .collect()
        };
        self.create_block_with_tx_hashes(&tx_hashes);
    }

    /// Creates a block with the given transaction.
    /// Transactions that are in the mempool will be ignored.
    ///
    /// # Panics
    ///
    /// - Panics if given transaction has been already committed to the blockchain.
    pub fn create_block_with_transaction<T: Transaction>(&mut self, tx: T) {
        self.create_block_with_transactions(txvec![tx]);
    }

    /// Creates block with the specified transactions. The transactions must be previously
    /// sent to the node via API or directly put into the `channel()`.
    ///
    /// # Panics
    ///
    /// - Panics in the case any of transaction hashes are not in the mempool.
    pub fn create_block_with_tx_hashes(&mut self, tx_hashes: &[crypto::Hash]) {
        self.poll_events();

        {
            let txs = self.mempool();
            for hash in tx_hashes {
                assert!(txs.contains_key(hash));
            }
        }

        self.do_create_block(tx_hashes);
    }

    /// Creates block with all transactions in the mempool.
    pub fn create_block(&mut self) {
        self.poll_events();

        let tx_hashes: Vec<_> = self.mempool().keys().cloned().collect();

        self.do_create_block(&tx_hashes);
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

    /// Returns the test node memory pool handle.
    pub fn mempool(&self) -> RwLockReadGuard<BTreeMap<crypto::Hash, Box<Transaction>>> {
        self.mempool.read().expect(
            "Can't read transactions from the mempool.",
        )
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
