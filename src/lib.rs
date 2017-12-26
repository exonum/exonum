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
//! use exonum::crypto::{gen_keypair, PublicKey};
//! use exonum::blockchain::{Block, Schema, Service, Transaction};
//! use exonum::messages::{Message, RawTransaction};
//! use exonum::storage::Fork;
//! use exonum::encoding;
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! // Simple service implementation.
//!
//! const SERVICE_ID: u16 = 1;
//! const TX_TIMESTAMP_ID: u16 = 1;
//!
//! message! {
//!     struct TxTimestamp {
//!         const TYPE = SERVICE_ID;
//!         const ID = TX_TIMESTAMP_ID;
//!         const SIZE = 40;
//!
//!         field from: &PublicKey [0 => 32]
//!         field msg: &str [32 => 40]
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
//!     fn execute(&self, _fork: &mut Fork) {}
//!
//!     fn info(&self) -> serde_json::Value {
//!         serde_json::to_value(self).unwrap()
//!     }
//! }
//!
//! impl Service for TimestampingService {
//!     fn service_name(&self) -> &'static str {
//!         "timestamping"
//!     }
//!
//!     fn service_id(&self) -> u16 {
//!         SERVICE_ID
//!     }
//!
//!     fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
//!         let trans: Box<Transaction> = match raw.message_type() {
//!             TX_TIMESTAMP_ID => Box::new(TxTimestamp::from_raw(raw)?),
//!             _ => {
//!                 return Err(encoding::Error::IncorrectMessageType {
//!                     message_type: raw.message_type(),
//!                 });
//!             }
//!         };
//!         Ok(trans)
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
//!     let blocks: Vec<Block> = api.get(ApiKind::Explorer, "v1/blocks?count=10");
//!     assert_eq!(blocks.len(), 3);
//!     api.get::<serde_json::Value>(
//!         ApiKind::System,
//!         &format!("v1/transactions/{}", tx1.hash().to_string()),
//!     );
//! }
//! ```

#![deny(missing_debug_implementations, missing_docs)]

extern crate bodyparser;
extern crate exonum;
extern crate futures;
extern crate iron;
extern crate iron_test;
extern crate mount;
extern crate router;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use futures::Stream;
use futures::executor::{self, Spawn};
use futures::sync::mpsc;
use iron::IronError;
use iron::headers::{ContentType, Headers};
use iron::status::StatusClass;
use iron_test::{request, response};
use mount::Mount;
use router::Router;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard};
use std::fmt;

use exonum::blockchain::{Blockchain, ConsensusConfig, GenesisConfig, Schema as CoreSchema,
                         Service, StoredConfiguration, Transaction, ValidatorKeys};
use exonum::crypto;
use exonum::helpers::{Height, Round, ValidatorId};
use exonum::messages::{Message, Precommit, Propose};
use exonum::node::{ApiSender, ExternalMessage, State as NodeState, TransactionSend, TxPool};
use exonum::storage::{MemoryDB, Snapshot};

#[macro_use]
mod macros;
mod checkpoint_db;
pub mod compare;
mod greedy_fold;

#[doc(hidden)]
pub use greedy_fold::GreedilyFoldable;
pub use compare::ComparableSnapshot;

use checkpoint_db::{CheckpointDb, CheckpointDbHandler};

/// Emulated test network.
#[derive(Debug)]
pub struct TestNetwork {
    us: TestNode,
    validators: Vec<TestNode>,
}

impl TestNetwork {
    /// Creates a new emulated network.
    pub fn new(validator_count: u16) -> Self {
        let validators = (0..validator_count)
            .map(ValidatorId)
            .map(TestNode::new_validator)
            .collect::<Vec<_>>();

        let us = validators[0].clone();
        TestNetwork { validators, us }
    }

    /// Returns the node in the emulated network, from whose perspective the testkit operates.
    pub fn us(&self) -> &TestNode {
        &self.us
    }

    /// Returns a slice of all validators in the network.
    pub fn validators(&self) -> &[TestNode] {
        &self.validators
    }

    /// Returns config encoding the network structure usable for creating the genesis block of
    /// a blockchain.
    pub fn genesis_config(&self) -> GenesisConfig {
        GenesisConfig::new(self.validators.iter().map(TestNode::public_keys))
    }

    /// Updates the test network by the new set of nodes.
    pub fn update<I: IntoIterator<Item = TestNode>>(&mut self, mut us: TestNode, validators: I) {
        let validators = validators
            .into_iter()
            .enumerate()
            .map(|(id, mut validator)| {
                let validator_id = ValidatorId(id as u16);
                validator.change_role(Some(validator_id));
                if us.public_keys().consensus_key == validator.public_keys().consensus_key {
                    us.change_role(Some(validator_id));
                }
                validator
            })
            .collect::<Vec<_>>();
        self.validators = validators;
        self.us.clone_from(&us);
    }

    /// Returns service public key of the validator with given id.
    pub fn service_public_key_of(&self, id: ValidatorId) -> Option<&crypto::PublicKey> {
        self.validators().get(id.0 as usize).map(|x| {
            &x.service_public_key
        })
    }

    /// Returns consensus public key of the validator with given id.
    pub fn consensus_public_key_of(&self, id: ValidatorId) -> Option<&crypto::PublicKey> {
        self.validators().get(id.0 as usize).map(|x| {
            &x.consensus_public_key
        })
    }
}

/// An emulated node in the test network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestNode {
    consensus_secret_key: crypto::SecretKey,
    consensus_public_key: crypto::PublicKey,
    service_secret_key: crypto::SecretKey,
    service_public_key: crypto::PublicKey,
    validator_id: Option<ValidatorId>,
}

impl TestNode {
    /// Creates a new auditor.
    pub fn new_auditor() -> Self {
        let (consensus_public_key, consensus_secret_key) = crypto::gen_keypair();
        let (service_public_key, service_secret_key) = crypto::gen_keypair();

        TestNode {
            consensus_secret_key,
            consensus_public_key,
            service_secret_key,
            service_public_key,
            validator_id: None,
        }
    }

    /// Creates a new validator with the given id.
    pub fn new_validator(validator_id: ValidatorId) -> Self {
        let (consensus_public_key, consensus_secret_key) = crypto::gen_keypair();
        let (service_public_key, service_secret_key) = crypto::gen_keypair();

        TestNode {
            consensus_secret_key,
            consensus_public_key,
            service_secret_key,
            service_public_key,
            validator_id: Some(validator_id),
        }
    }

    /// Constructs a new node from the given keypairs.
    pub fn from_parts(
        consensus_keypair: (crypto::PublicKey, crypto::SecretKey),
        service_keypair: (crypto::PublicKey, crypto::SecretKey),
        validator_id: Option<ValidatorId>,
    ) -> TestNode {
        TestNode {
            consensus_public_key: consensus_keypair.0,
            consensus_secret_key: consensus_keypair.1,
            service_public_key: service_keypair.0,
            service_secret_key: service_keypair.1,
            validator_id,
        }
    }

    /// Creates a `Propose` message signed by this validator.
    pub fn create_propose(
        &self,
        height: Height,
        last_hash: &crypto::Hash,
        tx_hashes: &[crypto::Hash],
    ) -> Propose {
        Propose::new(
            self.validator_id.expect(
                "An attempt to create propose from a non-validator node.",
            ),
            height,
            Round::first(),
            last_hash,
            tx_hashes,
            &self.consensus_secret_key,
        )
    }

    /// Creates a `Precommit` message signed by this validator.
    pub fn create_precommit(&self, propose: &Propose, block_hash: &crypto::Hash) -> Precommit {
        use std::time::SystemTime;

        Precommit::new(
            self.validator_id.expect(
                "An attempt to create propose from a non-validator node.",
            ),
            propose.height(),
            propose.round(),
            &propose.hash(),
            block_hash,
            SystemTime::now(),
            &self.consensus_secret_key,
        )
    }

    /// Returns public keys of the node.
    pub fn public_keys(&self) -> ValidatorKeys {
        ValidatorKeys {
            consensus_key: self.consensus_public_key,
            service_key: self.service_public_key,
        }
    }

    /// Returns the current validator id of node if it is validator of the test network.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_id
    }

    /// Changes node role.
    pub fn change_role(&mut self, role: Option<ValidatorId>) {
        self.validator_id = role;
    }

    /// Returns the service keypair.
    pub fn service_keypair(&self) -> (&crypto::PublicKey, &crypto::SecretKey) {
        (&self.service_public_key, &self.service_secret_key)
    }
}

impl From<TestNode> for ValidatorKeys {
    fn from(node: TestNode) -> Self {
        node.public_keys()
    }
}

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
/// #    fn service_name(&self) -> &'static str {
/// #        "documentation"
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
    us: TestNode,
    validators: Vec<TestNode>,
    services: Vec<Box<Service>>,
}

impl fmt::Debug for TestKitBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TestKitBuilder")
            .field("us", &self.us)
            .field("validators", &self.validators)
            .field(
                "services",
                &self.services
                    .iter()
                    .map(|x| x.service_name())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl TestKitBuilder {
    /// Creates testkit for the validator node.
    pub fn validator() -> Self {
        let us = TestNode::new_validator(ValidatorId(0));
        TestKitBuilder {
            validators: vec![us.clone()],
            services: Vec::new(),
            us,
        }
    }

    /// Creates testkit for the auditor node.
    pub fn auditor() -> Self {
        let us = TestNode::new_auditor();
        TestKitBuilder {
            validators: vec![TestNode::new_validator(ValidatorId(0))],
            services: Vec::new(),
            us,
        }
    }

    /// Sets the number of validator nodes in the test network.
    pub fn with_validators(mut self, validators_count: u16) -> Self {
        assert!(
            validators_count > 0,
            "At least one validator should be present in the network."
        );
        let additional_validators = (self.validators.len() as u16..validators_count)
            .map(ValidatorId)
            .map(TestNode::new_validator);
        self.validators.extend(additional_validators);
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

    /// Creates the testkit.
    pub fn create(self) -> TestKit {
        crypto::init();
        TestKit::assemble(
            self.services,
            TestNetwork {
                us: self.us,
                validators: self.validators,
            },
        )
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
    mempool: TxPool,
    cfg_proposal: Option<ConfigurationProposalState>,
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
    fn assemble(services: Vec<Box<Service>>, network: TestNetwork) -> Self {
        let api_channel = mpsc::channel(1_000);
        let api_sender = ApiSender::new(api_channel.0.clone());

        let db = CheckpointDb::new(MemoryDB::new());
        let db_handler = db.handler();

        let mut blockchain = Blockchain::new(
            Box::new(db),
            services,
            *network.us().service_keypair().0,
            network.us().service_keypair().1.clone(),
            api_sender.clone(),
        );

        let genesis = network.genesis_config();
        blockchain.create_genesis_block(genesis.clone()).unwrap();

        let mempool = Arc::new(RwLock::new(BTreeMap::new()));
        let events_stream: Box<Stream<Item = (), Error = ()> + Send + Sync> = {
            let blockchain = blockchain.clone();
            let mempool = Arc::clone(&mempool);
            Box::new(api_channel.1.and_then(move |event| {
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
                    ExternalMessage::PeerAdd(_) => { /* Ignored */ }
                }
                future_ok(())
            }))
        };

        TestKit {
            blockchain,
            db_handler,
            api_sender,
            events_stream,
            network,
            mempool: Arc::clone(&mempool),
            cfg_proposal: None,
        }
    }

    /// Creates a mounting point for public APIs used by the blockchain.
    fn public_api_mount(&self) -> Mount {
        self.blockchain.mount_public_api()
    }

    /// Creates a mounting point for public APIs used by the blockchain.
    fn private_api_mount(&self) -> Mount {
        self.blockchain.mount_private_api()
    }

    /// Creates an instance of `TestKitApi` to test the API provided by services.
    pub fn api(&self) -> TestKitApi {
        TestKitApi::new(self)
    }

    /// Polls the *existing* events from the event loop until exhaustion. Does not wait
    /// until new events arrive.
    pub fn poll_events(&mut self) -> Option<Result<(), ()>> {
        let mut spawn = executor::spawn(self.events_stream.by_ref().greedy_fold((), |_, _| {}));
        spawn.wait_stream()
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
    /// # use exonum::blockchain::{Service, Transaction};
    /// # use exonum::messages::RawTransaction;
    /// # use exonum::encoding;
    /// # use exonum_testkit::{TestKit, TestKitBuilder};
    /// #
    /// # type FromRawResult = Result<Box<Transaction>, encoding::Error>;
    /// # pub struct MyService;
    /// # impl Service for MyService {
    /// #    fn service_name(&self) -> &'static str {
    /// #        "documentation"
    /// #    }
    /// #    fn service_id(&self) -> u16 {
    /// #        0
    /// #    }
    /// #    fn tx_from_raw(&self, _raw: RawTransaction) -> FromRawResult {
    /// #        unimplemented!();
    /// #    }
    /// # }
    /// #
    /// # message! {
    /// #     struct MyTransaction {
    /// #         const TYPE = 0;
    /// #         const ID = 0;
    /// #         const SIZE = 40;
    /// #         field from: &exonum::crypto::PublicKey [0 => 32]
    /// #         field msg: &str [32 => 40]
    /// #     }
    /// # }
    /// # impl Transaction for MyTransaction {
    /// #     fn verify(&self) -> bool { true }
    /// #     fn execute(&self, _: &mut exonum::storage::Fork) {}
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
                        self.network_mut().update(
                            cfg_proposal.us,
                            cfg_proposal.validators,
                        );
                    } else {
                        self.cfg_proposal = Some(Committed(cfg_proposal));
                    }
                }
            }
        }
    }

    /// Returns a reference to the scheduled configutation proposal, or `None` if
    /// there is no such proposal.
    pub fn next_configuration(&self) -> Option<&TestNetworkConfiguration> {
        use ConfigurationProposalState::*;

        self.cfg_proposal.as_ref().map(|p| match *p {
            Committed(ref proposal) |
            Uncommitted(ref proposal) => proposal,
        })
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
                    let txid = tx.hash();
                    assert!(
                        !schema.transactions().contains(&txid),
                        "Transaction is already committed: {:?}",
                        tx
                    );
                    mempool.insert(txid, tx);
                    txid
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
    /// - Panics if validator with the given id is absent in test network.
    pub fn validator(&self, id: ValidatorId) -> &TestNode {
        &self.network.validators[id.0 as usize]
    }

    /// Returns sufficient number of validators for the Byzantine Fault Toulerance consensus.
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
        &self.network().validators[0]
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
        TestNetworkConfiguration::from_parts(
            self.network().us().clone(),
            self.network().validators().into(),
            stored_configuration,
        )
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
    /// use exonum::helpers::{Height, ValidatorId};
    /// use exonum_testkit::TestKitBuilder;
    /// use exonum::blockchain::Schema;
    /// use exonum::storage::StorageValue;
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
        Box::new(stream.for_each(|_| future_ok(())))
    }
}

/// A configuration of the test network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestNetworkConfiguration {
    us: TestNode,
    validators: Vec<TestNode>,
    stored_configuration: StoredConfiguration,
}

// A new configuration proposal state.
#[derive(Debug)]
enum ConfigurationProposalState {
    Uncommitted(TestNetworkConfiguration),
    Committed(TestNetworkConfiguration),
}

impl TestNetworkConfiguration {
    fn from_parts(
        us: TestNode,
        validators: Vec<TestNode>,
        mut stored_configuration: StoredConfiguration,
    ) -> Self {
        let prev_hash = exonum::storage::StorageValue::hash(&stored_configuration);
        stored_configuration.previous_cfg_hash = prev_hash;
        TestNetworkConfiguration {
            us,
            validators,
            stored_configuration,
        }
    }

    /// Returns the node from whose perspective the testkit operates.
    pub fn us(&self) -> &TestNode {
        &self.us
    }

    /// Modifies the node from whose perspective the testkit operates.
    pub fn set_us(&mut self, us: TestNode) {
        self.us = us;
        self.update_our_role();
    }

    /// Returns the test network validators.
    pub fn validators(&self) -> &[TestNode] {
        self.validators.as_ref()
    }

    /// Returns the current consensus configuration.
    pub fn consensus_configuration(&self) -> &ConsensusConfig {
        &self.stored_configuration.consensus
    }

    /// Return the height, starting from which this configuration becomes actual.
    pub fn actual_from(&self) -> Height {
        self.stored_configuration.actual_from
    }

    /// Modifies the height, starting from which this configuration becomes actual.
    pub fn set_actual_from(&mut self, actual_from: Height) {
        self.stored_configuration.actual_from = actual_from;
    }

    /// Modifies the current consensus configuration.
    pub fn set_consensus_configuration(&mut self, consensus: ConsensusConfig) {
        self.stored_configuration.consensus = consensus;
    }

    /// Modifies the validators list.
    pub fn set_validators<I>(&mut self, validators: I)
    where
        I: IntoIterator<Item = TestNode>,
    {
        self.validators = validators
            .into_iter()
            .enumerate()
            .map(|(idx, mut node)| {
                node.change_role(Some(ValidatorId(idx as u16)));
                node
            })
            .collect();
        self.stored_configuration.validator_keys = self.validators
            .iter()
            .cloned()
            .map(ValidatorKeys::from)
            .collect();
        self.update_our_role();
    }

    /// Returns the configuration for service with the given identifier.
    pub fn service_config<D>(&self, id: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        let value = self.stored_configuration.services.get(id).expect(
            "Unable to find configuration for service",
        );
        serde_json::from_value(value.clone()).unwrap()
    }

    /// Modifies the configuration of the service with the given identifier.
    pub fn set_service_config<D>(&mut self, id: &str, config: D)
    where
        D: Serialize,
    {
        let value = serde_json::to_value(config).unwrap();
        self.stored_configuration.services.insert(id.into(), value);
    }

    /// Returns the resulting exonum blockchain configuration.
    pub fn stored_configuration(&self) -> &StoredConfiguration {
        &self.stored_configuration
    }

    fn update_our_role(&mut self) {
        let validator_id = self.validators
            .iter()
            .position(|x| {
                x.public_keys().service_key == self.us.service_public_key
            })
            .map(|x| ValidatorId(x as u16));
        self.us.validator_id = validator_id;
    }
}

/// Kind of public or private REST API of an Exonum node.
///
/// `ApiKind` allows to use `get*` and `post*` methods of [`TestKitApi`] more safely.
///
/// [`TestKitApi`]: struct.TestKitApi.html
#[derive(Debug)]
pub enum ApiKind {
    /// `api/system` endpoints of the built-in Exonum REST API.
    System,
    /// `api/explorer` endpoints of the built-in Exonum REST API.
    Explorer,
    /// Endpoints corresponding to a service with the specified string identifier.
    Service(&'static str),
}

impl ApiKind {
    fn into_prefix(self) -> String {
        match self {
            ApiKind::System => "api/system".to_string(),
            ApiKind::Explorer => "api/explorer".to_string(),
            ApiKind::Service(name) => format!("api/services/{}", name),
        }
    }
}

/// API encapsulation for the testkit. Allows to execute and synchronously retrieve results
/// for REST-ful endpoints of services.
pub struct TestKitApi {
    public_mount: Mount,
    private_mount: Mount,
    api_sender: ApiSender,
}

impl fmt::Debug for TestKitApi {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TestKitApi").finish()
    }
}

impl TestKitApi {
    /// Creates a new instance of API.
    fn new(testkit: &TestKit) -> Self {
        use std::sync::Arc;
        use exonum::api::{public, Api};

        let blockchain = &testkit.blockchain;

        TestKitApi {
            public_mount: {
                let mut mount = Mount::new();

                let service_mount = testkit.public_api_mount();
                mount.mount("api/services", service_mount);

                let mut router = Router::new();
                let pool = Arc::clone(&testkit.mempool);
                let system_api = public::SystemApi::new(pool, blockchain.clone());
                system_api.wire(&mut router);
                mount.mount("api/system", router);

                let mut router = Router::new();
                let explorer_api = public::ExplorerApi::new(blockchain.clone());
                explorer_api.wire(&mut router);
                mount.mount("api/explorer", router);

                mount
            },

            private_mount: {
                let mut mount = Mount::new();

                let service_mount = testkit.private_api_mount();
                mount.mount("api/services", service_mount);

                mount
            },

            api_sender: testkit.api_sender.clone(),
        }
    }

    /// Returns the mounting point for public APIs. Useful for intricate testing not covered
    /// by `get*` and `post*` functions.
    pub fn public_mount(&self) -> &Mount {
        &self.public_mount
    }

    /// Returns the mounting point for private APIs. Useful for intricate testing not covered
    /// by `get*` and `post*` functions.
    pub fn private_mount(&self) -> &Mount {
        &self.private_mount
    }

    /// Sends a transaction to the node via `ApiSender`.
    pub fn send<T: Transaction>(&self, transaction: T) {
        self.api_sender.send(Box::new(transaction)).expect(
            "Cannot send transaction",
        );
    }

    fn get_internal<D>(mount: &Mount, url: &str, expect_error: bool) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        let status_class = if expect_error {
            StatusClass::ClientError
        } else {
            StatusClass::Success
        };

        let url = format!("http://localhost:3000/{}", url);
        let resp = request::get(&url, Headers::new(), mount);
        let resp = if expect_error {
            // Support either "normal" or erroneous responses.
            // For example, `Api.not_found_response()` returns the response as `Ok(..)`.
            match resp {
                Ok(resp) => resp,
                Err(IronError { response, .. }) => response,
            }
        } else {
            resp.expect("Got unexpected `Err(..)` response")
        };

        if let Some(ref status) = resp.status {
            if status.class() != status_class {
                panic!("Unexpected response status: {:?}", status);
            }
        } else {
            panic!("Response status not set");
        }

        let resp = response::extract_body_to_string(resp);
        serde_json::from_str(&resp).unwrap()
    }

    /// Gets information from a public endpoint of the node.
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during request processing (e.g., the requested endpoint is
    ///  unknown), or if the response has a non-20x response status.
    pub fn get<D>(&self, kind: ApiKind, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::get_internal(
            &self.public_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            false,
        )
    }

    /// Gets information from a private endpoint of the node.
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during request processing (e.g., the requested endpoint is
    ///  unknown), or if the response has a non-20x response status.
    pub fn get_private<D>(&self, kind: ApiKind, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::get_internal(
            &self.public_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            false,
        )
    }

    /// Gets an error from a public endpoint of the node.
    ///
    /// # Panics
    ///
    /// - Panics if the response has a non-40x response status.
    pub fn get_err<D>(&self, kind: ApiKind, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::get_internal(
            &self.public_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            true,
        )
    }

    fn post_internal<T, D>(mount: &Mount, endpoint: &str, data: &T) -> D
    where
        T: Serialize,
        for<'de> D: Deserialize<'de>,
    {
        let url = format!("http://localhost:3000/{}", endpoint);
        let resp = request::post(
            &url,
            {
                let mut headers = Headers::new();
                headers.set(ContentType::json());
                headers
            },
            &serde_json::to_string(&data).expect("Cannot serialize data to JSON"),
            mount,
        ).expect("Cannot send data");

        let resp = response::extract_body_to_string(resp);
        serde_json::from_str(&resp).expect("Cannot parse result")
    }

    /// Posts a transaction to the service using the public API. The returned value is the result
    /// of synchronous transaction processing, which includes running the API shim
    /// and `Transaction.verify()`. `Transaction.execute()` is not run until the transaction
    /// gets to a block via one of `create_block*()` methods.
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during request processing (e.g., the requested endpoint is
    ///  unknown).
    pub fn post<T, D>(&self, kind: ApiKind, endpoint: &str, transaction: &T) -> D
    where
        T: Serialize,
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::post_internal(
            &self.public_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            transaction,
        )
    }

    /// Posts a transaction to the service using the private API. The returned value is the result
    /// of synchronous transaction processing, which includes running the API shim
    /// and `Transaction.verify()`. `Transaction.execute()` is not run until the transaction
    /// gets to a block via one of `create_block*()` methods.
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during request processing (e.g., the requested endpoint is
    ///  unknown).
    pub fn post_private<T, D>(&self, kind: ApiKind, endpoint: &str, transaction: &T) -> D
    where
        T: Serialize,
        for<'de> D: Deserialize<'de>,
    {
        TestKitApi::post_internal(
            &self.private_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            transaction,
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
