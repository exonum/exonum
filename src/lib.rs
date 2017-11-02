//! Test harness for Exonum blockchain framework, allowing to test service APIs synchronously
//! and in the same process as the harness.

#![deny(missing_docs)]

extern crate exonum;
extern crate futures;
extern crate mount;
extern crate iron;
extern crate iron_test;
extern crate router;
extern crate serde;
extern crate serde_json;

use std::collections::BTreeMap;

use exonum::blockchain::{ApiContext, Blockchain, Service, Transaction, GenesisConfig,
                         SharedNodeState, Schema as CoreSchema, ValidatorKeys};
use exonum::crypto;
// A bit hacky, `exonum::events` is hidden from docs.
use exonum::events::{Event as ExonumEvent, EventHandler};
use exonum::helpers::{Height, Round, ValidatorId};
use exonum::messages::{Message, Propose, Precommit};
use exonum::node::{ApiSender, NodeChannel, NodeHandler, Configuration, ListenerConfig,
                   ServiceConfig, DefaultSystemState, State as NodeState, TransactionSend};
use exonum::storage::{MemoryDB, Snapshot};
use futures::Stream;
use futures::executor;
use iron::headers::{Headers, ContentType};
use iron_test::{request, response};
use mount::Mount;
use router::Router;
use serde::{Serialize, Deserialize};

pub mod compare;
mod greedy_fold;

#[doc(hidden)]
pub use greedy_fold::GreedilyFoldable;
pub use compare::ComparableSnapshot;

const STATE_UPDATE_TIMEOUT: u64 = 10_000;

/// Emulated test network.
pub struct TestNetwork {
    validators: Vec<Validator>,
}

impl TestNetwork {
    /// Creates a new emulated network.
    pub fn new(validator_count: u16) -> Self {
        let mut validators = Vec::with_capacity(validator_count as usize);
        for i in 0..validator_count {
            validators.push(Validator::new(ValidatorId(i)));
        }
        TestNetwork { validators }
    }

    /// Returns the node in the emulated network, from whose perspective the harness operates.
    pub fn us(&self) -> &Validator {
        &self.validators[0]
    }

    /// Returns a slice of all validators in the network.
    pub fn validators(&self) -> &[Validator] {
        &self.validators
    }

    /// Returns config encoding the network structure usable for creating the genesis block of
    /// a blockchain.
    pub fn config(&self) -> GenesisConfig {
        GenesisConfig::new(self.validators
            .iter()
            .map(Validator::public_keys),
        )
    }
}

/// An emulated validator in the test network.
pub struct Validator {
    consensus_secret_key: crypto::SecretKey,
    consensus_public_key: crypto::PublicKey,
    service_secret_key: crypto::SecretKey,
    service_public_key: crypto::PublicKey,
    validator_id: ValidatorId,
}

impl Validator {
    /// Creates a new validator.
    pub fn new(validator_id: ValidatorId) -> Self {
        let (consensus_public_key, consensus_secret_key) = crypto::gen_keypair();
        let (service_public_key, service_secret_key) = crypto::gen_keypair();

        Validator {
            consensus_secret_key,
            consensus_public_key,
            service_secret_key,
            service_public_key,
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
            self.validator_id,
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
            self.validator_id,
            propose.height(),
            propose.round(),
            &propose.hash(),
            block_hash,
            SystemTime::now(),
            &self.consensus_secret_key,
        )
    }

    /// Returns public keys of the validator.
    pub fn public_keys(&self) -> ValidatorKeys {
        ValidatorKeys {
            consensus_key: self.consensus_public_key,
            service_key: self.service_public_key,
        }
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
        assert!(validator_count > 0, "Number of validators should be positive");
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
    handler: NodeHandler,
    channel: NodeChannel,
    api_context: ApiContext,
    network: TestNetwork,
}

impl TestHarness {
    /// Initializes a harness with a blockchain and a single-node network.
    pub fn new(blockchain: Blockchain) -> Self {
        TestHarness::assemble(
            blockchain,
            TestNetwork::new(1),
        )
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

    fn assemble(
        mut blockchain: Blockchain,
        network: TestNetwork,
    ) -> Self {
        let genesis = network.config();
        blockchain.create_genesis_block(genesis.clone()).unwrap();

        let listen_address = "0.0.0.0:2000".parse().unwrap();
        let api_state = SharedNodeState::new(STATE_UPDATE_TIMEOUT);
        let system_state = Box::new(DefaultSystemState(listen_address));
        let channel = NodeChannel::new(Default::default());
        let api_sender = ApiSender::new(channel.api_requests.0.clone());

        let (config, api_context) = {
            let our_node = network.us();

            (Configuration {
                listener: ListenerConfig {
                    consensus_public_key: our_node.consensus_public_key,
                    consensus_secret_key: our_node.consensus_secret_key.clone(),
                    whitelist: Default::default(),
                    address: listen_address,
                },
                service: ServiceConfig {
                    service_public_key: our_node.service_public_key,
                    service_secret_key: our_node.service_secret_key.clone(),
                },
                mempool: Default::default(),
                network: Default::default(),
                peer_discovery: vec![],
            }, ApiContext::from_parts(
                &blockchain,
                api_sender,
                &our_node.service_public_key,
                &our_node.service_secret_key,
            ))
        };

        let handler = NodeHandler::new(
            blockchain,
            listen_address,
            channel.node_sender(),
            system_state,
            config,
            api_state,
        );

        TestHarness {
            handler,
            channel,
            api_context,
            network,
        }
    }

    /// Returns the node state of the harness.
    pub fn state(&self) -> &NodeState {
        self.handler.state()
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
        let handler = &mut self.handler;
        let event_stream = self.channel.api_requests.1.by_ref().greedy_fold(
            (),
            |_, event| {
                handler.handle_event(ExonumEvent::Api(event))
            },
        );
        let mut event_exec = executor::spawn(event_stream);
        event_exec.wait_stream()
    }

    /// Returns a snapshot of the current blockchain state.
    pub fn snapshot(&self) -> Box<Snapshot> {
        self.handler.blockchain.snapshot()
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
        let height = self.state().height();

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

        let (_, patch) = self.handler.blockchain.create_patch(
            validator_id,
            height,
            &hashes,
            &transaction_map,
        );

        let mut fork = self.handler.blockchain.fork();
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
        let validator_id = self.state().validator_id().expect(
            "Tested node is not a validator",
        );
        let height = self.state().height();
        let last_hash = *self.state().last_hash();
        let round = Round::first();

        let handler = &mut self.handler;
        let (block_hash, patch) = handler.create_block(validator_id, height, tx_hashes);
        handler.state.add_block(
            block_hash,
            patch,
            tx_hashes.to_vec(),
            validator_id,
        );

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
        handler.commit(block_hash, precommits.iter(), Some(round));
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
            let txs = self.state().transactions().read().expect(
                "Cannot read transactions from node",
            );
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
            let txs = self.state().transactions().read().expect(
                "Cannot read transactions from node",
            );
            txs.keys().cloned().collect()
        };

        self.do_create_block(&tx_hashes);
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
        use exonum::api::{Api, public};

        let blockchain = &harness.handler.blockchain;

        HarnessApi {
            public_mount: {
                let mut mount = Mount::new();

                let service_mount = harness.public_api_mount();
                mount.mount("api/services", service_mount);

                let mut router = Router::new();
                let pool = harness.state().transactions().clone();
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

                //let harness_mount = harness.harness_mount();
                //mount.mount("api/harness", harness_mount);

                mount
            },

            api_sender: harness.api_context.node_channel().clone(),
        }
    }

    /// Sends a transaction to the node via `ApiSender`.
    pub fn send<T: Transaction>(&self, transaction: T) {
        self.api_sender.send(Box::new(transaction)).expect(
            "Cannot send transaction",
        );
    }

    fn get_internal<D>(mount: &Mount, url: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        let url = format!("http://localhost:3000/{}", url);
        let resp = request::get(&url, Headers::new(), mount).unwrap();
        let resp = response::extract_body_to_string(resp);
        // TODO: check status
        serde_json::from_str(&resp).unwrap()
    }

    #[doc(hidden)]
    pub fn get<D>(&self, kind: ApiKind, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        HarnessApi::get_internal(
            &self.public_mount,
            &format!("{}/{}", kind.into_prefix(), endpoint),
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
