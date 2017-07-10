//! Exonum node that performs consensus algorithm.
//!
//! For details about consensus message handling see messages module documentation.

// TODO: Move to the root `lib.rs` when all other things are documented.
#![deny(missing_docs)]

use router::Router;
use mount::Mount;
use iron::{Chain, Iron};
use toml::Value;

use std::io;
use std::net::SocketAddr;
use std::time::{SystemTime, Duration};
use std::collections::BTreeMap;
use std::thread;
use std::fmt;

use crypto::{self, PublicKey, SecretKey, Hash};
use events::{Events, Reactor, NetworkConfiguration, Event, EventsConfiguration, Channel,
             MioChannel, Network, EventLoop, Milliseconds, EventHandler, Result as EventsResult,
             Error as EventsError};
use blockchain::{SharedNodeState, Blockchain, Schema,
                    GenesisConfig, Transaction, ApiContext};
use messages::{Connect, RawMessage};
use explorer::ExplorerApi;
use api::Api;

use self::timeout_adjuster::TimeoutAdjuster;

pub use self::state::{State, Round, Height, RequestData, ValidatorId, TxPool, ValidatorState};
pub use self::whitelist::Whitelist;

mod basic;
mod consensus;
mod requests;
mod whitelist;
pub mod state; // TODO: temporary solution to get access to WAIT consts
pub mod timeout_adjuster;

const PROFILE_ENV_VARIABLE_NAME: &'static str = "EXONUM_PROFILE_FILENAME";
/// External messages.
#[derive(Debug)]
pub enum ExternalMessage {
    /// Add new connection
    PeerAdd(SocketAddr),
    /// Transaction that implements the `Transaction` trait.
    Transaction(Box<Transaction>),
}

/// Node timeout types.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeTimeout {
    /// Status timeout with the current height.
    Status(Height),
    /// Round timeout.
    Round(Height, Round),
    /// `RequestData` timeout.
    Request(RequestData, Option<PublicKey>),
    /// Propose timeout.
    Propose(Height, Round),
    /// Update api shared state.
    UpdateApiState,
    /// Exchange peers timeout.
    PeerExchange,
}

/// Transactions sender.
#[derive(Clone)]
pub struct ApiSender<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    inner: S,
}

/// Handler that that performs consensus algorithm.
pub struct NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    /// Timeout to update api state.
    pub state_update_timeout: usize,
    /// State of the `NodeHandler`.
    pub state: State,
    /// Shared api state
    pub api_state: SharedNodeState,
    /// Channel for messages and timeouts.
    pub channel: S,
    /// Blockchain.
    pub blockchain: Blockchain,
    /// Known peer addresses.
    // TODO: move this into peer exchange service
    pub peer_discovery: Vec<SocketAddr>,
    timeout_adjuster: Box<TimeoutAdjuster>
}

/// Service configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service public key.
    pub service_public_key: PublicKey,
    /// Service secret key.
    pub service_secret_key: SecretKey,
}

/// Listener config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListenerConfig {
    /// Public key.
    pub consensus_public_key: PublicKey,
    /// Secret key.
    pub consensus_secret_key: SecretKey,
    /// Whitelist.
    pub whitelist: Whitelist,
    /// Socket address.
    pub address: SocketAddr,
}

/// An api configuration options.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeApiConfig {
    /// Timeout to update api state.
    pub state_update_timeout: usize,
    /// Enable api endpoints for the `blockchain_explorer` on public api address.
    pub enable_blockchain_explorer: bool,
    /// Listen address for public api endpoints.
    pub public_api_address: Option<SocketAddr>,
    /// Listen address for private api endpoints.
    pub private_api_address: Option<SocketAddr>,
}

impl Default for NodeApiConfig {
    fn default() -> NodeApiConfig {
        NodeApiConfig {
            state_update_timeout: 10000,
            enable_blockchain_explorer: true,
            public_api_address: None,
            private_api_address: None,
        }
    }
}

/// Memory pool configuration parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryPoolConfig {
    /// Maximum number of uncommited transactions.
    pub tx_pool_capacity: usize,
    /// Sets the maximum number of messages that can be buffered on the event loop's 
    /// notification channel before a send will fail.
    pub events_pool_capacity: usize,
}

impl Default for MemoryPoolConfig {
    fn default() -> MemoryPoolConfig {
        MemoryPoolConfig {
            tx_pool_capacity: 100000,
            events_pool_capacity: 400000,
        }
    }
}

/// Configuration for the `Node`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Initial config that will be written in the first block.
    pub genesis: GenesisConfig,
    /// Network listening address.
    pub listen_address: SocketAddr,
    /// Remote Network address used by this node.
    pub external_address: Option<SocketAddr>,
    /// Network configuration.
    pub network: NetworkConfiguration,
    /// Peer addresses.
    pub peers: Vec<SocketAddr>,
    /// Consensus public key.
    pub consensus_public_key: PublicKey,
    /// Consensus secret key.
    pub consensus_secret_key: SecretKey,
    /// Service public key.
    pub service_public_key: PublicKey,
    /// Service secret key.
    pub service_secret_key: SecretKey,
    /// Node's whitelist.
    pub whitelist: Whitelist,
    /// Api configuration.
    pub api: NodeApiConfig,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
    /// Additional config, usable for services.
    pub services_configs: BTreeMap<String, Value>,
}

/// Configuration for the `NodeHandler`.
#[derive(Debug, Clone)]
pub struct Configuration {
    /// Current node socket address, public and secret keys.
    pub listener: ListenerConfig,
    /// Service configuration.
    pub service: ServiceConfig,
    /// Events configuration.
    pub events: EventsConfiguration,
    /// Network configuration.
    pub network: NetworkConfiguration,
    /// Known peer addresses.
    pub peer_discovery: Vec<SocketAddr>,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
}

/// Channel for messages and timeouts.
pub type NodeChannel = MioChannel<ExternalMessage, NodeTimeout>;

/// Node that contains handler (`NodeHandler`) and `NodeApiConfig`.
#[derive(Debug)]
pub struct Node {
    reactor: Events<NodeHandler<NodeChannel>>,
    api_options: NodeApiConfig,
}

impl<S> NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    /// Creates `NodeHandler` using specified `Configuration`.
    pub fn new(
        blockchain: Blockchain,
        external_address: SocketAddr,
        sender: S,
        config: Configuration,
        state_update_timeout: usize,
        ) -> Self
    {
        // FIXME: remove unwraps here, use FATAL log level instead
        let (last_hash, last_height) = {
            let block = blockchain.last_block();
            (block.hash(), block.height() + 1)
        };

        let snapshot = blockchain.snapshot();

        let stored = Schema::new(&snapshot).actual_configuration();
        info!("Creating a node with config: {:#?}", stored);

        let validator_id = stored
            .validator_keys
            .iter()
            .position(|pk| pk.consensus_key == config.listener.consensus_public_key)
            .map(|id| id as ValidatorId);
        info!("Validator id = '{:?}'", validator_id);
        let connect = Connect::new(&config.listener.consensus_public_key,
                                   external_address,
                                   sender.get_time(),
                                   &config.listener.consensus_secret_key);

        let mut whitelist = config.listener.whitelist;
        whitelist.set_validators(stored.validator_keys.iter().map(|x| x.consensus_key));
        let mut state = State::new(validator_id,
                               config.listener.consensus_public_key,
                               config.listener.consensus_secret_key,
                               config.service.service_public_key,
                               config.service.service_secret_key,
                               config.mempool.tx_pool_capacity,
                               whitelist,
                               stored,
                               connect,
                               last_hash,
                               last_height,
                               sender.get_time());

        let mut timeout_adjuster = Box::new(timeout_adjuster::Constant::default());
        let timeout = timeout_adjuster.adjust_timeout(&state, &*snapshot);
        state.set_propose_timeout(timeout);

        NodeHandler {
            state: state,
            channel: sender,
            blockchain: blockchain,
            timeout_adjuster: timeout_adjuster,
            peer_discovery: config.peer_discovery,
            state_update_timeout: state_update_timeout,
            api_state: SharedNodeState::new(),
        }
    }

    /// Return internal `SharedNodeState`
    pub fn api_state(&self) -> &SharedNodeState {
        &self.api_state
    } 
    /// Returns value of the `state_update_timeout`.
    pub fn state_update_timeout(&self) -> Milliseconds {
        self.state_update_timeout as u64
    }

    /// Sets new timeout adjuster.
    pub fn set_timeout_adjuster(&mut self, adjuster: Box<timeout_adjuster::TimeoutAdjuster>) {
        self.timeout_adjuster = adjuster;
    }

    /// Returns value of the `propose_timeout` field from the current `ConsensusConfig`.
    pub fn propose_timeout(&self) -> Milliseconds {
        self.state().consensus_config().propose_timeout
    }

    /// Returns value of the `round_timeout` field from the current `ConsensusConfig`.
    pub fn round_timeout(&self) -> Milliseconds {
        self.state().consensus_config().round_timeout
    }

    /// Returns value of the `status_timeout` field from the current `ConsensusConfig`.
    pub fn status_timeout(&self) -> Milliseconds {
        self.state().consensus_config().status_timeout
    }

    /// Returns value of the `peers_timeout` field from the current `ConsensusConfig`.
    pub fn peers_timeout(&self) -> Milliseconds {
        self.state().consensus_config().peers_timeout
    }

    /// Returns value of the `txs_block_limit` field from the current `ConsensusConfig`.
    pub fn txs_block_limit(&self) -> u32 {
        self.state().consensus_config().txs_block_limit
    }

    /// Returns `State` of the node.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Performs node initialization, so it starts consensus process from the first round.
    pub fn initialize(&mut self) {
        info!("Start listening address={}", self.channel.address());
        for address in &self.peer_discovery.clone() {
            if address == &self.channel.address() {
                continue;
            }
            self.connect(address);
            info!("Trying to connect with peer {}", address);
        }

        let round = 1;
        self.state.jump_round(round);
        info!("Jump to round {}", round);

        self.add_round_timeout();
        self.add_status_timeout();
        self.add_peer_exchange_timeout();
        self.add_update_api_state_timeout();
    }

    /// Sends the given message to a peer by its id.
    pub fn send_to_validator(&mut self, id: u32, message: &RawMessage) {
        // TODO: check validator id
        let public_key = self.state.validators()[id as usize].consensus_key;
        self.send_to_peer(public_key, message);
    }

    /// Sends the given message to a peer by its public key.
    pub fn send_to_peer(&mut self, public_key: PublicKey, message: &RawMessage) {
        if let Some(conn) = self.state.peers().get(&public_key) {
            trace!("Send to address: {}", conn.addr());
            self.channel.send_to(&conn.addr(), message.clone());
        } else {
            warn!("Hasn't connection with peer {:?}", public_key);
        }
    }

    /// Sends `RawMessage` to the specified address.
    pub fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        trace!("Send to address: {}", address);
        self.channel.send_to(address, message.clone());
    }

    /// Broadcasts given message to all peers.
    // TODO: use Into<RawMessage>
    pub fn broadcast(&mut self, message: &RawMessage) {
        for conn in self.state.peers().values() {
            trace!("Send to address: {}", conn.addr());
            self.channel.send_to(&conn.addr(), message.clone());
        }
    }

    /// Performs connection to the specified network address.
    pub fn connect(&mut self, address: &SocketAddr) {
        self.channel.connect(address);
    }

    /// Adds request timeout if it isn't already requested.
    pub fn request(&mut self, data: RequestData, peer: PublicKey) {
        let is_new = self.state.request(data.clone(), peer);
        if is_new {
            self.add_request_timeout(data, None);
        }
    }

    /// Adds `NodeTimeout::Round` timeout to the channel.
    pub fn add_round_timeout(&mut self) {
        let time = self.round_start_time(self.state.round() + 1);
        trace!("ADD ROUND TIMEOUT: time={:?}, height={}, round={}",
               time,
               self.state.height(),
               self.state.round());
        let timeout = NodeTimeout::Round(self.state.height(), self.state.round());
        self.channel.add_timeout(timeout, time);
    }

    /// Adds `NodeTimeout::Propose` timeout to the channel.
    pub fn add_propose_timeout(&mut self) {
        let adjusted_propose_timeout = self.state.propose_timeout();
        let time = self.round_start_time(self.state.round()) +
                   Duration::from_millis(adjusted_propose_timeout);

        trace!("ADD PROPOSE TIMEOUT: time={:?}, height={}, round={}",
               time,
               self.state.height(),
               self.state.round());
        let timeout = NodeTimeout::Propose(self.state.height(), self.state.round());
        self.channel.add_timeout(timeout, time);
    }

    /// Adds `NodeTimeout::Status` timeout to the channel.
    pub fn add_status_timeout(&mut self) {
        let time = self.channel.get_time() + Duration::from_millis(self.status_timeout());
        self.channel
            .add_timeout(NodeTimeout::Status(self.state.height()), time);
    }

    /// Adds `NodeTimeout::Request` timeout with `RequestData` to the channel.
    pub fn add_request_timeout(&mut self, data: RequestData, peer: Option<PublicKey>) {
        trace!("ADD REQUEST TIMEOUT");
        let time = self.channel.get_time() + data.timeout();
        self.channel
            .add_timeout(NodeTimeout::Request(data, peer), time);
    }

    /// Adds `NodeTimeout::PeerExchange` timeout to the channel.
    pub fn add_peer_exchange_timeout(&mut self) {
        trace!("ADD PEER EXCHANGE TIMEOUT");
        let time = self.channel.get_time() + Duration::from_millis(self.peers_timeout());
        self.channel.add_timeout(NodeTimeout::PeerExchange, time);
    }

    /// Adds `NodeTimeout::UpdateApiState` timeout to the channel.
    pub fn add_update_api_state_timeout(&mut self) {
        let time = self.channel.get_time() + Duration::from_millis(self.state_update_timeout());
        self.channel.add_timeout(NodeTimeout::UpdateApiState, time);
    }

    /// Returns hash of the last block.
    pub fn last_block_hash(&self) -> Hash {
        self.blockchain.last_block().hash()
    }

    /// Returns start time of the requested round.
    pub fn round_start_time(&self, round: Round) -> SystemTime {
        let ms = (round - 1) as u64 * self.round_timeout();
        self.state.height_start_time() + Duration::from_millis(ms)
    }
}

impl<S> EventHandler for NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    type Timeout = NodeTimeout;
    type ApplicationEvent = ExternalMessage;

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Connected(addr) => self.handle_connected(&addr),
            Event::Disconnected(addr) => self.handle_disconnected(&addr),
            Event::Incoming(raw) => self.handle_message(raw),
            Event::Error(_) => {}
        }
    }

    fn handle_application_event(&mut self, event: Self::ApplicationEvent) {
        match event {
            ExternalMessage::Transaction(tx) => {
                self.handle_incoming_tx(tx);
            }
            ExternalMessage::PeerAdd(address) => {
                info!("Send Connect message to {}", address);
                self.connect(&address);
            }
        }
    }

    fn handle_timeout(&mut self, timeout: Self::Timeout) {
        match timeout {
            NodeTimeout::Round(height, round) => self.handle_round_timeout(height, round),
            NodeTimeout::Request(data, peer) => self.handle_request_timeout(data, peer),
            NodeTimeout::Status(height) => self.handle_status_timeout(height),
            NodeTimeout::PeerExchange => self.handle_peer_exchange_timeout(),
            NodeTimeout::UpdateApiState => self.handle_update_api_state_timeout(),
            NodeTimeout::Propose(height, round) => self.handle_propose_timeout(height, round),
        }
    }
}

impl<S> fmt::Debug for NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NodeHandler {{ channel: Channel {{ .. }}, blockchain: {:?}, \
            peer_discovery: {:?}, timeout_adjuster: Box<TimeoutAdjuster> }}",
               self.blockchain, self.peer_discovery)
    }
}

/// `TransactionSend` represents interface for sending transactions. For details see `ApiSender`
/// implementation.
pub trait TransactionSend: Send + Sync {
    /// Sends transaction. This can include transaction verification.
    fn send(&self, tx: Box<Transaction>) -> EventsResult<()>;
}

impl<S> ApiSender<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    /// Creates new `ApiSender` with given channel.
    pub fn new(inner: S) -> ApiSender<S> {
        ApiSender { inner: inner }
    }
}

impl<S> TransactionSend for ApiSender<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    fn send(&self, tx: Box<Transaction>) -> EventsResult<()> {
        if !tx.verify() {
            return Err(EventsError::new("Unable to verify transaction"));
        }
        let msg = ExternalMessage::Transaction(tx);
        self.inner.post_event(msg)
    }
}

impl<T> fmt::Debug for ApiSender<T>
    where T: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("ApiSender { .. }")
    }
}

impl Node {
    /// Creates node for the given blockchain and node configuration.
    pub fn new(mut blockchain: Blockchain, node_cfg: NodeConfig) -> Self {
        crypto::init();

        if cfg!(feature="flame_profile") {
            ::profiler::init_handler(
                ::std::env::var(PROFILE_ENV_VARIABLE_NAME)
                .expect(
                    &format!("You compiled exonum with profiling support, but {}",
                     PROFILE_ENV_VARIABLE_NAME)))
        };

        blockchain.create_genesis_block(node_cfg.genesis.clone()).unwrap();


        let mut events_cfg = EventsConfiguration::default();
        events_cfg.notify_capacity(node_cfg.mempool.events_pool_capacity);
        let config = Configuration {
            listener: ListenerConfig {
                consensus_public_key: node_cfg.consensus_public_key,
                consensus_secret_key: node_cfg.consensus_secret_key,
                whitelist: node_cfg.whitelist,
                address: node_cfg.listen_address,
            },
            service: ServiceConfig {
                service_public_key: node_cfg.service_public_key,
                service_secret_key: node_cfg.service_secret_key,
            },
            mempool: node_cfg.mempool,
            network: node_cfg.network,
            events: events_cfg,
            peer_discovery: node_cfg.peers,
        };

        let external_address = 
        if let Some(v) = node_cfg.external_address
        {
            v
        } else {
            warn!("Could not find 'external_address' in the config, using 'listen_address'");
            node_cfg.listen_address
        };

        let network = Network::with_config(node_cfg.listen_address, config.network);
        let event_loop = EventLoop::configured(config.events.clone()).unwrap();
        let channel = NodeChannel::new(node_cfg.listen_address, event_loop.channel());
        let worker = NodeHandler::new(blockchain,
                                        external_address,
                                        channel,
                                        config,
                                        node_cfg.api.state_update_timeout);
        Node {
            reactor: Events::with_event_loop(network, worker, event_loop),
            api_options: node_cfg.api,
        }
    }

    /// Launches only consensus messages handler.
    /// This may be used if you want to customize api with the `ApiContext`.
    pub fn run_handler(&mut self) -> io::Result<()> {
        self.reactor.bind()?;
        self.reactor.handler_mut().initialize();
        self.reactor.run()
    }

    /// A generic implementation that launches `Node` and optionally creates threads
    /// for public and private api handlers.
    /// Explorer api prefix is `/api/explorer`
    /// Public api prefix is `/api/services/{service_name}`
    /// Private api prefix is `/api/services/{service_name}`
    pub fn run(&mut self) -> io::Result<()> {
        let blockchain = self.handler().blockchain.clone();

        let private_config_api_thread = match self.api_options.private_api_address {
            Some(listen_address) => {
                let api_context = ApiContext::new(self);
                let mut mount = Mount::new();
                mount.mount("api/services", api_context.mount_private_api());

                let thread = thread::spawn(move || {
                                               info!("Private exonum api started on {}",
                                                     listen_address);
                                               let chain = Chain::new(mount);
                                               Iron::new(chain).http(listen_address).unwrap();
                                           });
                Some(thread)
            }
            None => None,
        };
        let public_config_api_thread = match self.api_options.public_api_address {
            Some(listen_address) => {
                let api_context = ApiContext::new(self);
                let mut mount = Mount::new();
                mount.mount("api/services", api_context.mount_public_api());

                if self.api_options.enable_blockchain_explorer {

                    let pool = self.state().transactions().clone();
                    let shared_api_state = self.handler().api_state().clone();
                    let mut router = Router::new();
                    let explorer_api = ExplorerApi::new(blockchain, pool, shared_api_state);
                    explorer_api.wire(&mut router);
                    mount.mount("api/explorer", router);
                }

                let thread = thread::spawn(move || {
                                               info!("Public exonum api started on {}",
                                                     listen_address);

                                               let chain = Chain::new(mount);
                                               Iron::new(chain).http(listen_address).unwrap();
                                           });
                Some(thread)
            }
            None => None,
        };

        self.run_handler()?;

        if let Some(private_config_api_thread) = private_config_api_thread {
            private_config_api_thread.join().unwrap();
        }
        if let Some(public_config_api_thread) = public_config_api_thread {
            public_config_api_thread.join().unwrap();
        }

        Ok(())
    }

    /// Returns `State`.
    pub fn state(&self) -> &State {
        self.reactor.handler().state()
    }

    /// Returns `NodeHandler`.
    pub fn handler(&self) -> &NodeHandler<NodeChannel> {
        self.reactor.handler()
    }

    /// Returns channel.
    pub fn channel(&self) -> ApiSender<NodeChannel> {
        ApiSender::new(self.reactor.handler().channel.clone())
    }
}
