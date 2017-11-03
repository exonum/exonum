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

use exonum::blockchain::{ApiContext, Blockchain, Service, Transaction, GenesisConfig,
                         SharedNodeState, ValidatorKeys};
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
use iron::IronError;
use iron::headers::{Headers, ContentType};
use iron::status::StatusClass;
use iron_test::{request, response};
use mount::Mount;
use router::Router;
use serde::{Serialize, Deserialize};

mod checkpoint_db;
pub mod compare;
mod greedy_fold;

#[doc(hidden)]
pub use greedy_fold::GreedilyFoldable;
pub use compare::ComparableSnapshot;

use checkpoint_db::{CheckpointDb, CheckpointDbHandler};

const STATE_UPDATE_TIMEOUT: u64 = 10_000;

/// Macro allowing to create `Vec<Box<Transaction>>` from transaction references. Can be used for
/// `TestHarness.probe_all()`, among other things.
///
/// As the macro syntax implies, the transactions are not consumed; they are rather cloned before
/// being put into `Box`es.
///
/// # Examples
///
/// ```ignore
/// let tx = TxCreateWallet::new(..);
/// let other_tx = TxTransfer::new(..);
/// let txs = txvec![&tx, &other_tx];
/// ```
#[macro_export]
macro_rules! txvec {
    ($(&$tx:ident),+ $(,)*) => {
        vec![$(Box::new($tx.clone()) as Box<exonum::blockchain::Transaction>,)+]
    };
}

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
        GenesisConfig::new(self.validators.iter().map(Validator::public_keys))
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
    db_handler: CheckpointDbHandler<MemoryDB>,
    validator_count: u16,
}

impl TestHarnessBuilder {
    fn with_services<I>(services: I) -> Self
    where
        I: IntoIterator<Item = Box<Service>>,
    {
        let db = CheckpointDb::new(MemoryDB::new());
        let db_handler = db.handler();
        let blockchain = Blockchain::new(Box::new(db), services.into_iter().collect());
        TestHarnessBuilder {
            blockchain,
            db_handler,
            validator_count: 1,
        }
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
            self.db_handler.clone(),
            TestNetwork::new(self.validator_count),
        )
    }
}

/// Harness for testing blockchain services. It offers simple network configuration emulation
/// (with no real network setup).
pub struct TestHarness {
    handler: NodeHandler,
    db_handler: CheckpointDbHandler<MemoryDB>,
    channel: NodeChannel,
    api_context: ApiContext,
    network: TestNetwork,
}

impl TestHarness {
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
        db_handler: CheckpointDbHandler<MemoryDB>,
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

            (
                Configuration {
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
                },
                ApiContext::from_parts(
                    &blockchain,
                    api_sender,
                    &our_node.service_public_key,
                    &our_node.service_secret_key,
                ),
            )
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
            db_handler,
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

    /// Rolls the blockchain back for a certain number of blocks
    pub fn rollback(&mut self, blocks: usize) {
        assert!(
            (blocks as u64) < self.state().height().0,
            "Cannot rollback past genesis block"
        );
        self.db_handler.rollback(blocks);
    }

    /// Executes a list of transactions given the current state of the blockchain, but does not
    /// commit execution results to the blockchain. The execution result is the same
    /// as if transactions were included into a new block; for example,
    /// transactions included into one of previous blocks do not lead to any state changes.
    ///
    /// # Panics
    ///
    /// If there are duplicate transactions.
    pub fn probe_all<I>(&mut self, transactions: I) -> Box<Snapshot>
    where
        I: IntoIterator<Item = Box<Transaction>>,
    {
        use std::collections::BTreeSet;
        use std::iter::FromIterator;

        let transactions = transactions.into_iter();
        let mut tx_hashes = Vec::with_capacity(transactions.size_hint().0);

        let api = self.api();
        for tx in transactions {
            tx_hashes.push(tx.hash());
            api.send_boxed(tx);
        }
        self.poll_events();

        let tx_hashes: Vec<_> = {
            let mempool = self.state().transactions().read().expect(
                "Cannot acquire read lock on mempool",
            );
            tx_hashes
                .into_iter()
                .filter(|h| mempool.contains_key(h))
                .collect()
        };
        let tx_hash_set = BTreeSet::from_iter(tx_hashes.clone());
        assert_eq!(
            tx_hash_set.len(),
            tx_hashes.len(),
            "Duplicate transactions in probe"
        );

        self.do_create_block(&tx_hashes);

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
        use std::sync::Arc;
        use exonum::api::{Api, public};

        let blockchain = &harness.handler.blockchain;

        HarnessApi {
            public_mount: {
                let mut mount = Mount::new();

                let service_mount = harness.public_api_mount();
                mount.mount("api/services", service_mount);

                let mut router = Router::new();
                let pool = Arc::clone(harness.state().transactions());
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
        self.send_boxed(Box::new(transaction));
    }

    /// Sends a transaction to the node via `ApiSender`.
    pub fn send_boxed(&self, transaction: Box<Transaction>) {
        self.api_sender.send(transaction).expect(
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
