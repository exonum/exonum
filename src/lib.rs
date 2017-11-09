//! Test harness for Exonum blockchain framework, allowing to test service APIs synchronously
//! and in the same process as the harness.

#![deny(missing_docs)]

extern crate exonum;
extern crate futures;
extern crate iron;
extern crate iron_test;
extern crate mount;
extern crate router;
extern crate serde;
extern crate serde_json;

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard};

use exonum::blockchain::{ApiContext, Blockchain, ConsensusConfig, GenesisConfig,
                         Schema as CoreSchema, Service, ServiceContext, StoredConfiguration,
                         Transaction, ValidatorKeys};
use exonum::crypto;
use exonum::helpers::{Height, Round, ValidatorId};
use exonum::messages::{Message, Precommit, Propose};
use exonum::node::{ApiSender, ExternalMessage, NodeChannel, State as NodeState, TransactionSend,
                   TxPool};
use exonum::storage::{MemoryDB, Snapshot};

use futures::Stream;
use futures::executor;
use iron::IronError;
use iron::headers::{ContentType, Headers};
use iron::status::StatusClass;
use iron_test::{request, response};
use mount::Mount;
use router::Router;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod compare;
mod greedy_fold;

#[doc(hidden)]
pub use greedy_fold::GreedilyFoldable;
pub use compare::ComparableSnapshot;

/// Emulated test network.
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

    /// Returns the node in the emulated network, from whose perspective the harness operates.
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
}

/// An emulated node in the test network.
#[derive(Debug, Clone, PartialEq)]
pub struct TestNode {
    consensus_secret_key: crypto::SecretKey,
    consensus_public_key: crypto::PublicKey,
    service_secret_key: crypto::SecretKey,
    service_public_key: crypto::PublicKey,
    validator_id: Option<ValidatorId>,
}

impl TestNode {
    /// Creates a new auditor.
    pub fn new() -> Self {
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

    /// Change node role.
    pub fn change_role(&mut self, role: Option<ValidatorId>) {
        self.validator_id = role;
    }
}

/// Current state of the test node.
pub struct TestNodeState {
    node: TestNode,
    snapshot: Box<Snapshot>,
    actual_configuration: StoredConfiguration,
    api_sender: ApiSender,
}

impl TestNodeState {
    /// Creates a new node state for the given node with the given snapshot of the storage.
    pub fn new(node: TestNode, snapshot: Box<Snapshot>, api_sender: ApiSender) -> Self {
        let actual_configuration = CoreSchema::new(&snapshot).actual_configuration();
        TestNodeState {
            node,
            snapshot,
            actual_configuration,
            api_sender,
        }
    }

    /// Updates the snapshot of the storage.
    fn update(&mut self, node: TestNode, snapshot: Box<Snapshot>) {
        self.node = node;
        self.actual_configuration = CoreSchema::new(&snapshot).actual_configuration();
        self.snapshot = snapshot;
    }

    /// Returns sufficient number of validators for the Byzantine Fault Toulerance consensus.
    pub fn majority_count(&self) -> usize {
        NodeState::byzantine_majority_count(self.validators().len())
    }

    /// Returns hash of the latest commited block.
    pub fn last_hash(&self) -> crypto::Hash {
        CoreSchema::new(&self.snapshot).last_block().unwrap().hash()
    }

    /// Returns consensus public key of the validator with given id.
    pub fn consensus_public_key_of(&self, id: ValidatorId) -> Option<&crypto::PublicKey> {
        self.validators().get(id.0 as usize).map(
            |x| &x.consensus_key,
        )
    }

    /// Returns service public key of the validator with given id.
    pub fn service_public_key_of(&self, id: ValidatorId) -> Option<&crypto::PublicKey> {
        self.validators().get(id.0 as usize).map(|x| &x.service_key)
    }
}

impl<'a> ServiceContext for TestNodeState {
    fn validator_id(&self) -> Option<ValidatorId> {
        self.node.validator_id()
    }

    fn snapshot(&self) -> &Snapshot {
        self.snapshot.as_ref()
    }

    fn height(&self) -> Height {
        CoreSchema::new(&self.snapshot).last_height().unwrap()
    }

    fn round(&self) -> Round {
        Round(1)
    }

    fn validators(&self) -> &[ValidatorKeys] {
        &self.actual_configuration.validator_keys
    }

    fn public_key(&self) -> &crypto::PublicKey {
        &self.node.service_public_key
    }

    fn secret_key(&self) -> &crypto::SecretKey {
        &self.node.service_secret_key
    }

    fn actual_consensus_config(&self) -> &ConsensusConfig {
        &self.actual_configuration.consensus
    }

    fn actual_service_config(&self, service: &Service) -> &Value {
        &self.actual_configuration
            .services
            .get(service.service_name())
            .expect("An attempt to get configuration from nonexistent service")
    }

    fn api_sender(&self) -> &ApiSender {
        &self.api_sender
    }
}

/// Builder for `TestHarness`.
pub struct TestHarnessBuilder {
    blockchain: Blockchain,
    validator_count: u16,
}

impl TestHarnessBuilder {
    fn with_blockchain(blockchain: Blockchain) -> Self {
        TestHarnessBuilder {
            blockchain,
            validator_count: 1,
        }
    }

    fn with_services<I>(services: I) -> Self
    where
        I: IntoIterator<Item = Box<Service>>,
    {
        let db = MemoryDB::new();
        let blockchain = Blockchain::new(Box::new(db), services.into_iter().collect());
        TestHarnessBuilder::with_blockchain(blockchain)
    }

    /// Sets the validator count to be used in the harness emulation.
    pub fn validators(&mut self, validator_count: u16) -> &mut Self {
        assert!(
            validator_count > 0,
            "Number of validators should be positive"
        );
        self.validator_count = validator_count;
        self
    }

    /// Creates the harness.
    pub fn create(&self) -> TestHarness {
        crypto::init();
        TestHarness::assemble(
            self.blockchain.clone(),
            TestNetwork::new(self.validator_count),
        )
    }
}

/// Harness for testing blockchain services. It offers simple network configuration emulation
/// (with no real network setup).
pub struct TestHarness {
    blockchain: Blockchain,
    channel: NodeChannel,
    service_context: TestNodeState,
    api_context: ApiContext,
    network: TestNetwork,
    mempool: TxPool,
}

impl TestHarness {
    /// Initializes a harness with a blockchain and a single-node network.
    pub fn new(blockchain: Blockchain) -> Self {
        TestHarness::assemble(blockchain, TestNetwork::new(1))
    }

    /// Initializes a harness builder with a blockchain.
    pub fn with_blockchain(blockchain: Blockchain) -> TestHarnessBuilder {
        TestHarnessBuilder::with_blockchain(blockchain)
    }

    /// Initializes a harness with a blockchain that hosts a given set of services.
    /// The blockchain uses `MemoryDB` for storage.
    pub fn with_services<I>(services: I) -> TestHarnessBuilder
    where
        I: IntoIterator<Item = Box<Service>>,
    {
        TestHarnessBuilder::with_services(services)
    }

    fn assemble(mut blockchain: Blockchain, network: TestNetwork) -> Self {
        let genesis = network.genesis_config();
        blockchain.create_genesis_block(genesis.clone()).unwrap();

        let channel = NodeChannel::new(Default::default());

        let api_context = {
            let our_node = network.us();
            ApiContext::from_parts(
                &blockchain,
                ApiSender::new(channel.api_requests.0.clone()),
                &our_node.service_public_key,
                &our_node.service_secret_key,
            )
        };

        let service_context = TestNodeState::new(
            network.us().clone(),
            blockchain.snapshot(),
            ApiSender::new(channel.api_requests.0.clone()),
        );

        TestHarness {
            blockchain,
            channel,
            api_context,
            service_context,
            network,
            mempool: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Returns the node state of the harness.
    pub fn state(&self) -> &TestNodeState {
        &self.service_context
    }

    /// Creates a mounting point for public APIs used by the blockchain.
    fn public_api_mount(&self) -> Mount {
        self.api_context.mount_public_api()
    }

    /// Creates a mounting point for public APIs used by the blockchain.
    fn private_api_mount(&self) -> Mount {
        self.api_context.mount_private_api()
    }

    /// Creates an instance of `HarnessApi` to test the API provided by services.
    pub fn api(&self) -> HarnessApi {
        HarnessApi::new(self)
    }

    /// Polls the *existing* events from the event loop until exhaustion. Does not wait
    /// until new events arrive.
    pub fn poll_events(&mut self) -> Option<Result<(), ()>> {
        // XXX: Creating a new executor each time seems very inefficient, but sharing
        // a single executor seems to be impossible
        // because `handler` needs to be borrowed mutably into the closure. Use `RefCell`?
        let mempool = Arc::clone(&self.mempool);
        let snapshot = self.blockchain.snapshot();
        let schema = CoreSchema::new(snapshot);
        let event_stream = self.channel.api_requests.1.by_ref().greedy_fold(
            (),
            |_, event| {
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
            },
        );
        let mut event_exec = executor::spawn(event_stream);
        event_exec.wait_stream()
    }

    /// Returns a snapshot of the current blockchain state.
    pub fn snapshot(&self) -> Box<Snapshot> {
        self.blockchain.snapshot()
    }

    /// Executes a list of transactions given the current state of the blockchain, but does not
    /// commit execution results to the blockchain. The execution result is the same
    /// as if transactions were included into a new block; for example,
    /// transactions included into one of previous blocks do not lead to any state changes.
    ///
    /// # Panics
    ///
    /// If there are duplicate transactions.
    pub fn probe_all(&self, transactions: Vec<Box<Transaction>>) -> Box<Snapshot> {
        let validator_id = self.state().validator_id().expect(
            "Tested node is not a validator",
        );
        let height = self.height();

        let (transaction_map, hashes) = {
            let mut transaction_map = BTreeMap::new();
            let mut hashes = Vec::with_capacity(transactions.len());

            let core_schema = CoreSchema::new(self.snapshot());
            let committed_txs = core_schema.transactions();

            for tx in transactions {
                let hash = tx.hash();
                if committed_txs.contains(&hash) {
                    continue;
                }

                hashes.push(hash);
                transaction_map.insert(hash, tx);
            }

            assert_eq!(
                hashes.len(),
                transaction_map.len(),
                "Duplicate transactions in probe"
            );

            (transaction_map, hashes)
        };

        let (_, patch) = self.blockchain.create_patch(
            validator_id,
            height,
            &hashes,
            &transaction_map,
        );

        let mut fork = self.blockchain.fork();
        fork.merge(patch);
        Box::new(fork)
    }

    /// Executes a transaction given the current state of the blockchain but does not
    /// commit execution results to the blockchain. The execution result is the same
    /// as if a transaction was included into a new block; for example,
    /// a transaction included into one of previous blocks does not lead to any state changes.
    pub fn probe<T: Transaction>(&self, transaction: T) -> Box<Snapshot> {
        self.probe_all(vec![Box::new(transaction)])
    }

    fn do_create_block(&mut self, tx_hashes: &[crypto::Hash]) {
        let height = self.height();
        let last_hash = self.state().last_hash();

        let (block_hash, patch) = {
            let validator_id = self.state().validator_id().expect(
                "Tested node is not a validator",
            );
            let transactions = self.mempool();
            self.blockchain.create_patch(
                validator_id,
                height,
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

        let propose = self.network.us().create_propose(
            height,
            &last_hash,
            tx_hashes,
        );
        let precommits: Vec<_> = self.network
            .validators()
            .iter()
            .map(|v| v.create_precommit(&propose, &block_hash))
            .collect();

        let patch = {
            let mut fork = {
                let mut fork = self.blockchain.fork();
                fork.merge(patch.clone()); // FIXME: avoid cloning here
                fork
            };
            {
                let mut schema = CoreSchema::new(&mut fork);
                for precommit in precommits {
                    schema.precommits_mut(&block_hash).push(precommit.clone());
                }
            }
            fork.into_patch()
        };
        self.blockchain.merge(patch).unwrap();
        self.do_update();
    }

    // Update Harness state after commit block.
    fn do_update(&mut self) {
        // Update context
        self.service_context.update(
            self.network.us.clone(),
            self.blockchain.snapshot(),
        );
        // Invokes handle_commit event
        for service in self.blockchain.service_map().values() {
            service.handle_commit(&self.service_context);
        }
        self.poll_events();
    }

    /// Creates block with the specified transactions. The transactions must be previously
    /// sent to the node via API or directly put into the `channel()`.
    ///
    /// # Panics
    ///
    /// In the case any of transaction hashes are not in the mempool.
    pub fn create_block_with_transactions(&mut self, tx_hashes: &[crypto::Hash]) {
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

        let tx_hashes: Vec<_> = {
            let txs = self.mempool.read().expect(
                "Cannot read transactions from node",
            );
            txs.keys().cloned().collect()
        };

        self.do_create_block(&tx_hashes);
    }

    /// Returns the current height of the blockchain.
    pub fn height(&self) -> Height {
        self.state().height().next()
    }

    /// Returns the test node memory pool handle.
    pub fn mempool(&self) -> RwLockReadGuard<BTreeMap<crypto::Hash, Box<Transaction>>> {
        self.mempool.read().expect(
            "Can't read transactions from the mempool.",
        )
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub enum ApiKind {
    System,
    Explorer,
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

/// API encapsulation for the test harness. Allows to execute and synchronously retrieve results
/// for REST-ful endpoints of services.
pub struct HarnessApi {
    public_mount: Mount,
    private_mount: Mount,
    api_sender: ApiSender,
}

impl HarnessApi {
    /// Creates a new instance of Api.
    fn new(harness: &TestHarness) -> Self {
        use std::sync::Arc;
        use exonum::api::{public, Api};

        let blockchain = &harness.blockchain;

        HarnessApi {
            public_mount: {
                let mut mount = Mount::new();

                let service_mount = harness.public_api_mount();
                mount.mount("api/services", service_mount);

                let mut router = Router::new();
                let pool = Arc::clone(&harness.mempool);
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

                let service_mount = harness.private_api_mount();
                mount.mount("api/services", service_mount);

                mount
            },

            api_sender: harness.api_context.node_channel().clone(),
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
    pub fn get<D>(&self, kind: ApiKind, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        HarnessApi::get_internal(
            &self.public_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            false,
        )
    }

    /// Gets an error from a public endpoint of the node.
    pub fn get_err<D>(&self, kind: ApiKind, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        HarnessApi::get_internal(
            &self.public_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            true,
        )
    }

    fn post_internal<T, D>(mount: &Mount, endpoint: &str, transaction: &T) -> D
    where
        T: Transaction + Serialize,
        for<'de> D: Deserialize<'de>,
    {
        let url = format!("http://localhost:3000/{}", endpoint);
        let resp =
            request::post(
                &url,
                {
                    let mut headers = Headers::new();
                    headers.set(ContentType::json());
                    headers
                },
                &serde_json::to_string(&transaction).expect("Cannot serialize transaction to JSON"),
                mount,
            ).expect("Cannot send transaction");

        let resp = response::extract_body_to_string(resp);
        serde_json::from_str(&resp).expect("Cannot parse result")
    }

    /// Posts a transaction to the service using the public API. The returned value is the result
    /// of synchronous transaction processing, which includes running the API shim
    /// and `Transaction.verify()`. `Transaction.execute()` is not run until the transaction
    /// gets to a block via one of `create_block*()` methods.
    pub fn post<T, D>(&self, kind: ApiKind, endpoint: &str, transaction: &T) -> D
    where
        T: Transaction + Serialize,
        for<'de> D: Deserialize<'de>,
    {
        HarnessApi::post_internal(
            &self.public_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            transaction,
        )
    }

    /// Posts a transaction to the service using the private API. The returned value is the result
    /// of synchronous transaction processing, which includes running the API shim
    /// and `Transaction.verify()`. `Transaction.execute()` is not run until the transaction
    /// gets to a block via one of `create_block*()` methods.
    pub fn post_private<T, D>(&self, kind: ApiKind, endpoint: &str, transaction: &T) -> D
    where
        T: Transaction + Serialize,
        for<'de> D: Deserialize<'de>,
    {
        HarnessApi::post_internal(
            &self.private_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
            transaction,
        )
    }
}
