use router::Router;
use mount::Mount;
use iron::{Chain, Iron};

use std::io;
use std::net::SocketAddr;
use std::time::{SystemTime, Duration};
use std::thread;
use std::fmt;

use crypto::{PublicKey, SecretKey, Hash};
use events::{Events, Reactor, NetworkConfiguration, Event, EventsConfiguration, Channel,
             MioChannel, Network, EventLoop, Milliseconds, EventHandler, Result as EventsResult,
             Error as EventsError};
use blockchain::{Blockchain, Schema, GenesisConfig, Transaction, ApiContext};
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

#[derive(Debug)]
pub enum ExternalMessage {
    Transaction(Box<Transaction>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeTimeout {
    Status(Height),
    Round(u64, u32),
    Request(RequestData, Option<PublicKey>),
    Propose(u64, u32),
    PeerExchange,
}

#[derive(Clone)]
pub struct TxSender<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    inner: S,
}

pub struct NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    pub state: State,
    pub channel: S,
    pub blockchain: Blockchain,
    // TODO: move this into peer exchange service
    pub peer_discovery: Vec<SocketAddr>,
    timeout_adjuster: Box<TimeoutAdjuster>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListenerConfig {
    pub consensus_public_key: PublicKey,
    pub consensus_secret_key: SecretKey,
    pub service_public_key: PublicKey,
    pub service_secret_key: SecretKey,
    pub whitelist: Whitelist,
    pub address: SocketAddr,
}

/// An api configuration options
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeApiConfig {
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
            enable_blockchain_explorer: true,
            public_api_address: None,
            private_api_address: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub genesis: GenesisConfig,
    pub listen_address: SocketAddr,
    pub network: NetworkConfiguration,
    pub peers: Vec<SocketAddr>,
    pub consensus_public_key: PublicKey,
    pub consensus_secret_key: SecretKey,
    pub service_public_key: PublicKey,
    pub service_secret_key: SecretKey,
    pub whitelist: Whitelist,
    pub api: NodeApiConfig,
}

#[derive(Debug, Clone)]
pub struct Configuration {
    pub listener: ListenerConfig,
    pub events: EventsConfiguration,
    pub network: NetworkConfiguration,
    pub peer_discovery: Vec<SocketAddr>,
}

pub type NodeChannel = MioChannel<ExternalMessage, NodeTimeout>;

#[derive(Debug)]
pub struct Node {
    reactor: Events<NodeHandler<NodeChannel>>,
    api_options: NodeApiConfig,
}

impl<S> NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    pub fn new(blockchain: Blockchain, sender: S, config: Configuration) -> Self {
        // FIXME: remove unwraps here, use FATAL log level instead
        let (last_hash, last_height) = {
            let block = blockchain.last_block().unwrap();
            (block.hash(), block.height() + 1)
        };

        let stored = Schema::new(&blockchain.view())
            .actual_configuration()
            .unwrap();
        info!("Create node with config={:#?}", stored);

        let validator_id = stored
            .validators
            .iter()
            .position(|pk| pk == &config.listener.consensus_public_key)
            .map(|id| id as ValidatorId);
        info!("Validator={:#?}", validator_id);
        let connect = Connect::new(&config.listener.consensus_public_key,
                                   sender.address(),
                                   sender.get_time(),
                                   &config.listener.consensus_secret_key);

        let mut whitelist = config.listener.whitelist;
        whitelist.set_validators(stored.validators.iter().cloned()); 
        let mut state = State::new(validator_id,
                               config.listener.consensus_public_key,
                               config.listener.consensus_secret_key,
                               config.listener.service_public_key,
                               config.listener.service_secret_key,
                               whitelist,
                               stored,
                               connect,
                               last_hash,
                               last_height,
                               sender.get_time());

        let mut timeout_adjuster = Box::new(timeout_adjuster::Constant::default());
        let timeout = timeout_adjuster.adjust_timeout(&state, blockchain.view());
        state.set_propose_timeout(timeout);

        NodeHandler {
            state: state,
            channel: sender,
            blockchain: blockchain,
            peer_discovery: config.peer_discovery,
            timeout_adjuster: timeout_adjuster,
        }
    }

    pub fn set_timeout_adjuster(&mut self, adjuster: Box<timeout_adjuster::TimeoutAdjuster>) {
        self.timeout_adjuster = adjuster;
    }

    pub fn propose_timeout(&self) -> Milliseconds {
        self.state().consensus_config().propose_timeout
    }

    pub fn round_timeout(&self) -> Milliseconds {
        self.state().consensus_config().round_timeout
    }

    pub fn status_timeout(&self) -> Milliseconds {
        self.state().consensus_config().status_timeout
    }

    pub fn peers_timeout(&self) -> Milliseconds {
        self.state().consensus_config().peers_timeout
    }

    pub fn txs_block_limit(&self) -> u32 {
        self.state().consensus_config().txs_block_limit
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn initialize(&mut self) {
        info!("Start listening address={}", self.channel.address());
        for address in &self.peer_discovery.clone() {
            if address == &self.channel.address() {
                continue;
            }
            self.connect(address);
            info!("Try to connect with peer {}", address);
        }

        let round = 1;
        self.state.jump_round(round);
        info!("Jump to round {}", round);

        self.add_round_timeout();
        self.add_status_timeout();
        self.add_peer_exchange_timeout();
    }

    pub fn send_to_validator(&mut self, id: u32, message: &RawMessage) {
        // TODO: check validator id
        let public_key = self.state.validators()[id as usize];
        self.send_to_peer(public_key, message);
    }

    pub fn send_to_peer(&mut self, public_key: PublicKey, message: &RawMessage) {
        if let Some(conn) = self.state.peers().get(&public_key) {
            trace!("Send to addr: {}", conn.addr());
            self.channel.send_to(&conn.addr(), message.clone());
        } else {
            warn!("Hasn't connection with peer {:?}", public_key);
        }
    }

    pub fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        trace!("Send to addr: {}", address);
        self.channel.send_to(address, message.clone());
    }

    // TODO: use Into<RawMessage>
    pub fn broadcast(&mut self, message: &RawMessage) {
        for conn in self.state.peers().values() {
            trace!("Send to addr: {}", conn.addr());
            self.channel.send_to(&conn.addr(), message.clone());
        }
    }

    pub fn connect(&mut self, address: &SocketAddr) {
        self.channel.connect(address);
    }

    pub fn request(&mut self, data: RequestData, peer: PublicKey) {
        let is_new = self.state.request(data.clone(), peer);
        if is_new {
            self.add_request_timeout(data, None);
        }
    }

    pub fn add_round_timeout(&mut self) {
        let time = self.round_start_time(self.state.round() + 1);
        trace!("ADD ROUND TIMEOUT, time={:?}, height={}, round={}",
               time,
               self.state.height(),
               self.state.round());
        let timeout = NodeTimeout::Round(self.state.height(), self.state.round());
        self.channel.add_timeout(timeout, time);
    }

    pub fn add_propose_timeout(&mut self) {
        let adjusted_propose_timeout = self.state.propose_timeout();
        let time = self.round_start_time(self.state.round()) +
                   Duration::from_millis(adjusted_propose_timeout);

        trace!("ADD PROPOSE TIMEOUT, time={:?}, height={}, round={}",
               time,
               self.state.height(),
               self.state.round());
        let timeout = NodeTimeout::Propose(self.state.height(), self.state.round());
        self.channel.add_timeout(timeout, time);
    }

    pub fn add_status_timeout(&mut self) {
        let time = self.channel.get_time() + Duration::from_millis(self.status_timeout());
        self.channel
            .add_timeout(NodeTimeout::Status(self.state.height()), time);
    }

    pub fn add_request_timeout(&mut self, data: RequestData, peer: Option<PublicKey>) {
        trace!("ADD REQUEST TIMEOUT");
        let time = self.channel.get_time() + data.timeout();
        self.channel
            .add_timeout(NodeTimeout::Request(data, peer), time);
    }

    pub fn add_peer_exchange_timeout(&mut self) {
        let time = self.channel.get_time() + Duration::from_millis(self.peers_timeout());
        self.channel.add_timeout(NodeTimeout::PeerExchange, time);
    }

    pub fn last_block_hash(&self) -> Hash {
        self.blockchain.last_block().unwrap().hash()
    }

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
        }
    }

    fn handle_timeout(&mut self, timeout: Self::Timeout) {
        match timeout {
            NodeTimeout::Round(height, round) => self.handle_round_timeout(height, round),
            NodeTimeout::Request(data, peer) => self.handle_request_timeout(data, peer),
            NodeTimeout::Status(height) => self.handle_status_timeout(height),
            NodeTimeout::PeerExchange => self.handle_peer_exchange_timeout(),
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

pub trait TransactionSend: Send + Sync {
    fn send<T: Transaction>(&self, tx: T) -> EventsResult<()>;
}

impl<S> TxSender<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    pub fn new(inner: S) -> TxSender<S> {
        TxSender { inner: inner }
    }
}

impl<S> TransactionSend for TxSender<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    fn send<T: Transaction>(&self, tx: T) -> EventsResult<()> {
        // TODO remove double data convertation
        if !tx.verify() {
            return Err(EventsError::new("Unable to verify transaction"));
        }
        let msg = ExternalMessage::Transaction(Box::new(tx));
        self.inner.post_event(msg)
    }
}

impl<T> fmt::Debug for TxSender<T>
    where T: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("TxSender { .. }")
    }
}

impl Node {
    /// Creates node for the given blockchain and node configuration
    pub fn new(blockchain: Blockchain, node_cfg: NodeConfig) -> Node {
        blockchain
            .create_genesis_block(node_cfg.genesis.clone())
            .unwrap();

        let config = Configuration {
            listener: ListenerConfig {
                consensus_public_key: node_cfg.consensus_public_key,
                consensus_secret_key: node_cfg.consensus_secret_key,
                service_public_key: node_cfg.service_public_key,
                service_secret_key: node_cfg.service_secret_key,
                whitelist: node_cfg.whitelist,
                address: node_cfg.listen_address,
            },
            network: node_cfg.network,
            events: EventsConfiguration::default(),
            peer_discovery: node_cfg.peers,
        };
        let network = Network::with_config(node_cfg.listen_address, config.network);
        let event_loop = EventLoop::configured(config.events.clone()).unwrap();
        let channel = NodeChannel::new(node_cfg.listen_address, event_loop.channel());
        let worker = NodeHandler::new(blockchain, channel, config);
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
                    let mut router = Router::new();
                    let explorer_api = ExplorerApi::new(blockchain);
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

    pub fn state(&self) -> &State {
        self.reactor.handler().state()
    }

    pub fn handler(&self) -> &NodeHandler<NodeChannel> {
        self.reactor.handler()
    }

    pub fn channel(&self) -> TxSender<NodeChannel> {
        TxSender::new(self.reactor.handler().channel.clone())
    }
}
