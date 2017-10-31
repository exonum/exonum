//! Test harness for Exonum blockchain framework, allowing to test service APIs synchronously
//! and in the same process as the harness.

#![deny(missing_docs)]

extern crate exonum;
extern crate futures;
extern crate mount;
extern crate iron;
extern crate iron_test;
extern crate serde;
extern crate serde_json;

use std::collections::BTreeMap;

use exonum::blockchain::{ApiContext, Blockchain, Service, Transaction, GenesisConfig,
                         SharedNodeState};
use exonum::crypto;
// A bit hacky, `exonum::events` is hidden from docs.
use exonum::events::{Event as ExonumEvent, EventHandler};
use exonum::node::{ApiSender, NodeChannel, NodeHandler, Configuration, ListenerConfig,
                   ServiceConfig, DefaultSystemState, State as NodeState};
use exonum::storage::{MemoryDB, Snapshot};
use futures::Stream;
use futures::executor;
use iron::headers::{Headers, ContentType};
use iron_test::{request, response};
use mount::Mount;
use serde::{Serialize, Deserialize};

mod greedy_fold;
#[doc(hidden)]
pub use greedy_fold::GreedilyFoldable;

const STATE_UPDATE_TIMEOUT: u64 = 10_000;

/// Harness for testing blockchain with a single-node, no-network setup.
pub struct TestHarness {
    handler: NodeHandler,
    channel: NodeChannel,
    api_context: ApiContext,
}

impl TestHarness {
    /// Initializes a harness with a blockchain and no service-specific configuration.
    pub fn new(blockchain: Blockchain) -> Self {
        TestHarness::with_config(blockchain, BTreeMap::new())
    }

    /// Initializes a harness with a blockchain that hosts a given set of services.
    /// The blockchain uses `MemoryDB` for storage.
    pub fn with_services<I>(services: I) -> Self
    where
        I: IntoIterator<Item = Box<Service>>,
    {
        let db = MemoryDB::new();
        let blockchain = Blockchain::new(Box::new(db), services.into_iter().collect());
        TestHarness::new(blockchain)
    }

    /// Initializes a harness with a blockchain and a service-specific configuration.
    pub fn with_config(
        mut blockchain: Blockchain,
        _services_configs: BTreeMap<String, serde_json::Value>,
    ) -> Self {
        crypto::init();

        let (consensus_public_key, consensus_secret_key) = crypto::gen_keypair();
        let (service_public_key, service_secret_key) = crypto::gen_keypair();

        let validator_keys = exonum::blockchain::config::ValidatorKeys {
            consensus_key: consensus_public_key,
            service_key: service_public_key,
        };
        let genesis = GenesisConfig::new(vec![validator_keys].into_iter());

        blockchain.create_genesis_block(genesis.clone()).unwrap();

        let listen_address = "0.0.0.0:2000".parse().unwrap();
        let config = Configuration {
            listener: ListenerConfig {
                consensus_public_key,
                consensus_secret_key: consensus_secret_key.clone(),
                whitelist: Default::default(),
                address: listen_address,
            },
            service: ServiceConfig {
                service_public_key,
                service_secret_key: service_secret_key.clone(),
            },
            mempool: Default::default(),
            network: Default::default(),
            peer_discovery: vec![],
        };

        let api_state = SharedNodeState::new(STATE_UPDATE_TIMEOUT);
        let system_state = Box::new(DefaultSystemState(listen_address));
        let channel = NodeChannel::new(Default::default());
        let api_sender = ApiSender::new(channel.api_requests.0.clone());

        let api_context = ApiContext::from_parts(
            &blockchain,
            api_sender,
            &service_public_key,
            &service_secret_key,
        );

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
        }
    }

    /// Returns the node state of the harness.
    pub fn state(&self) -> &NodeState {
        self.handler.state()
    }

    /// Creates a mounting point for public APIs used by the blockchain.
    // TODO: consider making public
    fn public_api_mount(&self) -> Mount {
        self.api_context.mount_public_api()
    }

    /// Creates a mounting point for public APIs used by the blockchain.
    // TODO: consider making public
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

    fn do_create_block(&mut self, tx_hashes: &[crypto::Hash]) {
        use std::time::SystemTime;
        use exonum::helpers::Round;
        use exonum::messages::{Message, Propose, Precommit};

        let validator_id = self.state().validator_id().expect(
            "Tested node is not a validator",
        );
        let height = self.state().height();
        let last_hash = *self.state().last_hash();
        let consensus_secret_key = self.state().consensus_secret_key().clone();
        let round = Round::first();

        let handler = &mut self.handler;
        let (block_hash, patch) = handler.create_block(validator_id, height.next(), tx_hashes);
        handler.state.add_block(
            block_hash,
            patch,
            tx_hashes.to_vec(),
            validator_id,
        );

        let propose = Propose::new(
            validator_id,
            height,
            round,
            &last_hash,
            tx_hashes,
            &consensus_secret_key,
        );
        let precommit = Precommit::new(
            validator_id,
            height,
            round,
            &propose.hash(),
            &block_hash,
            SystemTime::now(),
            &consensus_secret_key,
        );

        handler.commit(block_hash, vec![precommit].iter(), Some(round));
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
        HarnessApi {
            public_mount: harness.public_api_mount(),
            private_mount: harness.private_api_mount(),
            api_sender: harness.api_context.node_channel().clone(),
        }
    }

    /// Sends a transaction to the node via `ApiSender`.
    pub fn send<T: Transaction>(&self, transaction: T) {
        self.api_sender.send(Box::new(transaction)).expect(
            "Cannot send transaction",
        );
    }

    fn do_get<D>(&self, service_name: &str, endpoint: &str, mount: &Mount) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        let url = format!("http://localhost:3000/{}/{}", service_name, endpoint);
        let resp = request::get(&url, Headers::new(), mount).unwrap();
        let resp = response::extract_body_to_string(resp);
        // TODO: check status
        serde_json::from_str(&resp).unwrap()
    }

    fn do_post<T, D>(&self, service_name: &str, endpoint: &str, mount: &Mount, transaction: &T) -> D
    where
        T: Transaction + Serialize,
        for<'de> D: Deserialize<'de>,
    {
        let url = format!("http://localhost:3000/{}/{}", service_name, endpoint);
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

    /// Retrieves information from a service using a `GET` method of the public API.
    pub fn get<D>(&self, service_name: &str, endpoint: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        self.do_get(service_name, endpoint, &self.public_mount)
    }

    /// Posts a transaction to the service using the public API. The returned value is the result
    /// of synchronous transaction processing, which includes running the API shim
    /// and `Transaction.verify()`. `Transaction.execute()` is not run until the transaction
    /// gets to a block via one of `create_block*()` methods.
    pub fn post<T, D>(&self, service_name: &str, endpoint: &str, transaction: &T) -> D
    where
        T: Transaction + Serialize,
        for<'de> D: Deserialize<'de>,
    {
        self.do_post(service_name, endpoint, &self.public_mount, transaction)
    }

    /// Posts a transaction to the service using the private API. The returned value is the result
    /// of synchronous transaction processing, which includes running the API shim
    /// and `Transaction.verify()`. `Transaction.execute()` is not run until the transaction
    /// gets to a block via one of `create_block*()` methods.
    pub fn post_private<T, D>(&self, service_name: &str, endpoint: &str, transaction: &T) -> D
    where
        T: Transaction + Serialize,
        for<'de> D: Deserialize<'de>,
    {
        self.do_post(service_name, endpoint, &self.private_mount, transaction)
    }
}
