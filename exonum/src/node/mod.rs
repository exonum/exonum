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

//! Exonum node that performs consensus algorithm.
//!
//! For details about consensus message handling see messages module documentation.

use toml::Value;
use router::Router;
use mount::Mount;
use iron::{Chain, Iron};
use futures::{Future, Sink};
use futures::sync::mpsc;
use tokio_core::reactor::Core;

use std::io;
use std::sync::Arc;
use std::thread;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};
use std::collections::BTreeMap;
use std::fmt;

use crypto::{self, Hash, PublicKey, SecretKey};
use blockchain::{ApiContext, Blockchain, GenesisConfig, Schema, SharedNodeState, Transaction};
use api::{private, public, Api};
use messages::{Connect, Message, RawMessage};
use events::{NetworkRequest, TimeoutRequest, NetworkEvent};
use events::{HandlerPart, NetworkConfiguration, NetworkPart, TimeoutsPart};
use events::error::{into_other, other_error, LogError, log_error};
use helpers::{Height, Milliseconds, Round, ValidatorId};

pub use self::state::{RequestData, State, TxPool, ValidatorState};
pub use self::whitelist::Whitelist;

mod events;
mod basic;
mod consensus;
mod requests;
mod whitelist;
pub mod state; // TODO: temporary solution to get access to WAIT consts (ECR-167)
pub mod timeout_adjuster;

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

/// A helper trait that provides the node with information about the state of the system such
/// as current time or listen address.
pub trait SystemStateProvider: 'static + ::std::fmt::Debug {
    /// Returns the current address that the node listens on.
    fn listen_address(&self) -> SocketAddr;
    /// Return the current system time.
    fn current_time(&self) -> SystemTime;
}

/// Transactions sender.
#[derive(Clone)]
pub struct ApiSender(pub mpsc::Sender<ExternalMessage>);

/// Handler that that performs consensus algorithm.
pub struct NodeHandler {
    /// State of the `NodeHandler`.
    pub state: State,
    /// Shared api state
    pub api_state: SharedNodeState,
    /// System state
    pub system_state: Box<SystemStateProvider>,
    /// Channel for messages and timeouts.
    pub channel: NodeSender,
    /// Blockchain.
    pub blockchain: Blockchain,
    /// Known peer addresses.
    // TODO: move this into peer exchange service
    pub peer_discovery: Vec<SocketAddr>,
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
            state_update_timeout: 10_000,
            enable_blockchain_explorer: true,
            public_api_address: None,
            private_api_address: None,
        }
    }
}

/// Events pool capacities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventsPoolCapacity {
    /// Maximum number of queued outgoing network messages.
    pub network_requests_capacity: usize,
    /// Maximum number of queued incoming network messages.
    pub network_events_capacity: usize,
    /// Maximum number of queued timeout requests.
    pub timeout_requests_capacity: usize,
    /// Maximum number of queued requests from api.
    pub api_requests_capacity: usize,
}

impl Default for EventsPoolCapacity {
    fn default() -> EventsPoolCapacity {
        EventsPoolCapacity {
            network_requests_capacity: 512,
            network_events_capacity: 512,
            timeout_requests_capacity: 128,
            api_requests_capacity: 1024,
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
    pub events_pool_capacity: EventsPoolCapacity,
}

impl Default for MemoryPoolConfig {
    fn default() -> MemoryPoolConfig {
        MemoryPoolConfig {
            tx_pool_capacity: 100_000,
            events_pool_capacity: EventsPoolCapacity::default(),
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
    /// Network configuration.
    pub network: NetworkConfiguration,
    /// Known peer addresses.
    pub peer_discovery: Vec<SocketAddr>,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
}

/// Channel for messages and timeouts requests.
#[derive(Debug, Clone)]
pub struct NodeSender {
    /// Timeout requests sender.
    pub timeout_requests: mpsc::Sender<TimeoutRequest>,
    /// Network requests sender.
    pub network_requests: mpsc::Sender<NetworkRequest>,
}

impl NodeHandler {
    /// Creates `NodeHandler` using specified `Configuration`.
    pub fn new(
        blockchain: Blockchain,
        external_address: SocketAddr,
        sender: NodeSender,
        system_state: Box<SystemStateProvider>,
        config: Configuration,
        api_state: SharedNodeState,
    ) -> Self {
        // FIXME: remove unwraps here, use FATAL log level instead
        let (last_hash, last_height) = {
            let block = blockchain.last_block();
            (block.hash(), block.height().next())
        };

        let snapshot = blockchain.snapshot();

        let stored = Schema::new(&snapshot).actual_configuration();
        info!("Creating a node with config: {:#?}", stored);

        let validator_id = stored
            .validator_keys
            .iter()
            .position(|pk| {
                pk.consensus_key == config.listener.consensus_public_key
            })
            .map(|id| ValidatorId(id as u16));
        info!("Validator id = '{:?}'", validator_id);
        let connect = Connect::new(
            &config.listener.consensus_public_key,
            external_address,
            system_state.current_time(),
            &config.listener.consensus_secret_key,
        );

        let mut whitelist = config.listener.whitelist;
        whitelist.set_validators(stored.validator_keys.iter().map(|x| x.consensus_key));
        let mut state = State::new(
            validator_id,
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
            system_state.current_time(),
        );

        // Adjust propose timeout for the first time.
        state.adjust_timeout(&*snapshot);

        NodeHandler {
            blockchain,
            api_state,
            system_state,
            state,
            channel: sender,
            peer_discovery: config.peer_discovery,
        }
    }

    /// Return internal `SharedNodeState`
    pub fn api_state(&self) -> &SharedNodeState {
        &self.api_state
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
        let addr = self.system_state.listen_address();
        info!("Start listening address={}", addr);
        for address in &self.peer_discovery.clone() {
            if address == &self.system_state.listen_address() {
                continue;
            }
            self.connect(address);
            info!("Trying to connect with peer {}", address);
        }

        let round = Round::first();
        self.state.jump_round(round);
        info!("Jump to round {}", round);

        self.add_round_timeout();
        self.add_status_timeout();
        self.add_peer_exchange_timeout();
        self.add_update_api_state_timeout();
    }

    /// Sends the given message to a peer by its id.
    pub fn send_to_validator(&self, id: u32, message: &RawMessage) {
        if id as usize >= self.state.validators().len() {
            error!("Invalid validator id: {}", id);
        } else {
            let public_key = self.state.validators()[id as usize].consensus_key;
            self.send_to_peer(public_key, message);
        }
    }

    /// Sends the given message to a peer by its public key.
    pub fn send_to_peer(&self, public_key: PublicKey, message: &RawMessage) {
        if let Some(conn) = self.state.peers().get(&public_key) {
            self.send_to_addr(&conn.addr(), message);
        } else {
            warn!("Hasn't connection with peer {:?}", public_key);
        }
    }

    /// Sends `RawMessage` to the specified address.
    pub fn send_to_addr(&self, address: &SocketAddr, message: &RawMessage) {
        trace!("Send to address: {}", address);
        let request = NetworkRequest::SendMessage(*address, message.clone());
        self.channel
            .network_requests
            .clone()
            .send(request)
            .wait()
            .log_error();
    }

    /// Broadcasts given message to all peers.
    pub fn broadcast(&mut self, message: &Message) {
        for conn in self.state.peers().values() {
            let addr = conn.addr();
            self.send_to_addr(&addr, message.raw());
        }
    }

    /// Performs connection to the specified network address.
    pub fn connect(&self, address: &SocketAddr) {
        let connect = self.state.our_connect_message().clone();
        self.send_to_addr(address, connect.raw());
    }

    /// Add timeout request.
    pub fn add_timeout(&self, timeout: NodeTimeout, time: SystemTime) {
        let request = TimeoutRequest(time, timeout);
        self.channel
            .timeout_requests
            .clone()
            .send(request)
            .wait()
            .log_error();
    }

    /// Adds request timeout if it isn't already requested.
    pub fn request(&mut self, data: RequestData, peer: PublicKey) {
        let is_new = self.state.request(data.clone(), peer);
        if is_new {
            self.add_request_timeout(data, None);
        }
    }

    /// Adds `NodeTimeout::Round` timeout to the channel.
    pub fn add_round_timeout(&self) {
        let time = self.round_start_time(self.state.round().next());
        trace!(
            "ADD ROUND TIMEOUT: time={:?}, height={}, round={}",
            time,
            self.state.height(),
            self.state.round()
        );
        let timeout = NodeTimeout::Round(self.state.height(), self.state.round());
        self.add_timeout(timeout, time);
    }

    /// Adds `NodeTimeout::Propose` timeout to the channel.
    pub fn add_propose_timeout(&self) {
        let adjusted_timeout = self.state.propose_timeout();
        let time = self.round_start_time(self.state.round()) +
            Duration::from_millis(adjusted_timeout);

        trace!(
            "ADD PROPOSE TIMEOUT: time={:?}, height={}, round={}",
            time,
            self.state.height(),
            self.state.round()
        );
        let timeout = NodeTimeout::Propose(self.state.height(), self.state.round());
        self.add_timeout(timeout, time);
    }

    /// Adds `NodeTimeout::Status` timeout to the channel.
    pub fn add_status_timeout(&self) {
        let time = self.system_state.current_time() + Duration::from_millis(self.status_timeout());
        self.add_timeout(NodeTimeout::Status(self.state.height()), time);
    }

    /// Adds `NodeTimeout::Request` timeout with `RequestData` to the channel.
    pub fn add_request_timeout(&self, data: RequestData, peer: Option<PublicKey>) {
        trace!("ADD REQUEST TIMEOUT");
        let time = self.system_state.current_time() + data.timeout();
        self.add_timeout(NodeTimeout::Request(data, peer), time);
    }

    /// Adds `NodeTimeout::PeerExchange` timeout to the channel.
    pub fn add_peer_exchange_timeout(&self) {
        trace!("ADD PEER EXCHANGE TIMEOUT");
        let time = self.system_state.current_time() + Duration::from_millis(self.peers_timeout());
        self.add_timeout(NodeTimeout::PeerExchange, time);
    }

    /// Adds `NodeTimeout::UpdateApiState` timeout to the channel.
    pub fn add_update_api_state_timeout(&self) {
        let time = self.system_state.current_time() +
            Duration::from_millis(self.api_state().state_update_timeout());
        self.add_timeout(NodeTimeout::UpdateApiState, time);
    }

    /// Returns hash of the last block.
    pub fn last_block_hash(&self) -> Hash {
        self.blockchain.last_block().hash()
    }

    /// Returns start time of the requested round.
    pub fn round_start_time(&self, round: Round) -> SystemTime {
        let previous_round: u64 = round.previous().into();
        let ms = previous_round * self.round_timeout();
        self.state.height_start_time() + Duration::from_millis(ms)
    }
}

impl fmt::Debug for NodeHandler {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "NodeHandler {{ channel: Channel {{ .. }}, blockchain: {:?}, peer_discovery: {:?} }}",
            self.blockchain,
            self.peer_discovery
        )
    }
}

/// `TransactionSend` represents interface for sending transactions. For details see `ApiSender`
/// implementation.
pub trait TransactionSend: Send + Sync {
    /// Sends transaction. This can include transaction verification.
    fn send(&self, tx: Box<Transaction>) -> io::Result<()>;
}

impl ApiSender {
    /// Creates new `ApiSender` with given channel.
    pub fn new(inner: mpsc::Sender<ExternalMessage>) -> ApiSender {
        ApiSender(inner)
    }

    /// Addr peer to peer list
    pub fn peer_add(&self, addr: SocketAddr) -> io::Result<()> {
        let msg = ExternalMessage::PeerAdd(addr);
        self.0.clone().send(msg).wait().map(drop).map_err(
            into_other,
        )
    }
}

impl TransactionSend for ApiSender {
    fn send(&self, tx: Box<Transaction>) -> io::Result<()> {
        if !tx.verify() {
            let msg = "Unable to verify transaction";
            return Err(io::Error::new(io::ErrorKind::Other, msg));
        }
        let msg = ExternalMessage::Transaction(tx);
        self.0.clone().send(msg).wait().map(drop).map_err(
            into_other,
        )
    }
}

impl fmt::Debug for ApiSender {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("ApiSender { .. }")
    }
}

/// Default system state provider implementation which just uses `SystemTime::now`
/// to get current time.
#[derive(Debug)]
pub struct DefaultSystemState(pub SocketAddr);

impl SystemStateProvider for DefaultSystemState {
    fn listen_address(&self) -> SocketAddr {
        self.0
    }
    fn current_time(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Channel between the `NodeHandler` and events source.
#[derive(Debug)]
pub struct NodeChannel {
    /// Channel for network requests.
    pub network_requests: (mpsc::Sender<NetworkRequest>, mpsc::Receiver<NetworkRequest>),
    /// Channel for timeout requests.
    pub timeout_requests: (mpsc::Sender<TimeoutRequest>, mpsc::Receiver<TimeoutRequest>),
    /// Channel for api requests.
    pub api_requests: (mpsc::Sender<ExternalMessage>, mpsc::Receiver<ExternalMessage>),
    /// Channel for network events.
    pub network_events: (mpsc::Sender<NetworkEvent>, mpsc::Receiver<NetworkEvent>),
    /// Channel for timeout events.
    pub timeout_events: (mpsc::Sender<NodeTimeout>, mpsc::Receiver<NodeTimeout>),
}

const PROFILE_ENV_VARIABLE_NAME: &'static str = "EXONUM_PROFILE_FILENAME";

/// Node that contains handler (`NodeHandler`) and `NodeApiConfig`.
#[derive(Debug)]
pub struct Node {
    api_options: NodeApiConfig,
    network_config: NetworkConfiguration,
    handler: NodeHandler,
    channel: NodeChannel,
}

impl NodeChannel {
    /// Creates `NodeChannel` with the given pool capacitites.
    pub fn new(buffer_sizes: EventsPoolCapacity) -> NodeChannel {
        NodeChannel {
            network_requests: mpsc::channel(buffer_sizes.network_requests_capacity),
            timeout_requests: mpsc::channel(buffer_sizes.timeout_requests_capacity),
            timeout_events: mpsc::channel(buffer_sizes.timeout_requests_capacity),
            api_requests: mpsc::channel(buffer_sizes.api_requests_capacity),
            network_events: mpsc::channel(buffer_sizes.network_events_capacity),
        }
    }

    /// Returns the channel for sending timeouts and networks requests.
    pub fn node_sender(&self) -> NodeSender {
        NodeSender {
            timeout_requests: self.timeout_requests.0.clone(),
            network_requests: self.network_requests.0.clone(),
        }
    }
}

impl Node {
    /// Creates node for the given blockchain and node configuration.
    pub fn new(mut blockchain: Blockchain, node_cfg: NodeConfig) -> Self {
        crypto::init();

        if cfg!(feature = "flame_profile") {
            ::exonum_profiler::init_handler(
                ::std::env::var(PROFILE_ENV_VARIABLE_NAME).expect(&format!(
                    "You compiled exonum with profiling support, but {}",
                    PROFILE_ENV_VARIABLE_NAME
                )),
            )
        };

        blockchain
            .create_genesis_block(node_cfg.genesis.clone())
            .unwrap();


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
            peer_discovery: node_cfg.peers,
        };

        let external_address = if let Some(v) = node_cfg.external_address {
            v
        } else {
            warn!("Could not find 'external_address' in the config, using 'listen_address'");
            node_cfg.listen_address
        };
        let api_state = SharedNodeState::new(node_cfg.api.state_update_timeout as u64);
        let system_state = Box::new(DefaultSystemState(node_cfg.listen_address));
        let channel = NodeChannel::new(config.mempool.events_pool_capacity.clone());
        let network_config = config.network;
        let handler = NodeHandler::new(
            blockchain,
            external_address,
            channel.node_sender(),
            system_state,
            config,
            api_state,
        );
        Node {
            api_options: node_cfg.api,
            handler,
            channel,
            network_config,
        }
    }

    /// Launches only consensus messages handler.
    /// This may be used if you want to customize api with the `ApiContext`.
    pub fn run_handler(mut self) -> io::Result<()> {
        self.handler.initialize();

        let (handler_part, network_part, timeouts_part) = self.into_reactor();

        let network_thread = thread::spawn(move || {
            let mut core = Core::new()?;
            let handle = core.handle();
            core.handle().spawn(
                timeouts_part.run(handle).map_err(log_error),
            );
            let network_handler = network_part.run(core.handle());
            core.run(network_handler).map(drop).map_err(|e| {
                other_error(&format!("An error in the `Network` thread occured: {}", e))
            })
        });

        let mut core = Core::new()?;
        core.run(handler_part.run()).map_err(|_| {
            other_error("An error in the `Handler` thread occured")
        })?;
        network_thread.join().unwrap()
    }

    /// A generic implementation that launches `Node` and optionally creates threads
    /// for public and private api handlers.
    /// Explorer api prefix is `/api/explorer`
    /// Public api prefix is `/api/services/{service_name}`
    /// Private api prefix is `/api/services/{service_name}`
    pub fn run(self) -> io::Result<()> {
        let blockchain = self.handler().blockchain.clone();
        let api_sender = self.channel();

        let private_config_api_thread = match self.api_options.private_api_address {
            Some(listen_address) => {
                let api_context = ApiContext::new(&self);
                let mut mount = Mount::new();
                mount.mount("api/services", api_context.mount_private_api());
                let shared_api_state = self.handler().api_state().clone();
                let mut router = Router::new();
                let node_info =
                    private::NodeInfo::new(blockchain.service_map().iter().map(|(_, s)| s));
                let system_api = private::SystemApi::new(
                    node_info,
                    blockchain.clone(),
                    shared_api_state,
                    api_sender,
                );
                system_api.wire(&mut router);
                mount.mount("api/system", router);

                let thread = thread::spawn(move || {
                    info!("Private exonum api started on {}", listen_address);
                    let chain = Chain::new(mount);
                    Iron::new(chain).http(listen_address).unwrap();
                });
                Some(thread)
            }
            None => None,
        };
        let public_config_api_thread = match self.api_options.public_api_address {
            Some(listen_address) => {
                let api_context = ApiContext::new(&self);
                let mut mount = Mount::new();
                mount.mount("api/services", api_context.mount_public_api());

                let mut router = Router::new();
                let pool = Arc::clone(self.state().transactions());
                let system_api = public::SystemApi::new(pool, blockchain.clone());
                system_api.wire(&mut router);
                mount.mount("api/system", router);
                if self.api_options.enable_blockchain_explorer {
                    let mut router = Router::new();
                    let explorer_api = public::ExplorerApi::new(blockchain);
                    explorer_api.wire(&mut router);
                    mount.mount("api/explorer", router);
                }

                let thread = thread::spawn(move || {
                    info!("Public exonum api started on {}", listen_address);

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

    fn into_reactor(self) -> (HandlerPart<NodeHandler>, NetworkPart, TimeoutsPart) {
        let connect_message = self.state().our_connect_message().clone();
        let (network_tx, network_rx) = self.channel.network_events;
        let timeout_requests_rx = self.channel.timeout_requests.1;

        let network_part = NetworkPart {
            our_connect_message: connect_message,
            listen_address: self.handler.system_state.listen_address(),
            network_requests: self.channel.network_requests,
            network_tx: network_tx,
            network_config: self.network_config,
        };

        let (timeout_tx, timeout_rx) = self.channel.timeout_events;
        let handler_part = HandlerPart {
            handler: self.handler,
            timeout_rx,
            network_rx: network_rx,
            api_rx: self.channel.api_requests.1,
        };

        let timeouts_part = TimeoutsPart {
            timeout_tx,
            timeout_requests_rx,
        };
        (handler_part, network_part, timeouts_part)
    }

    /// Returns `State`.
    pub fn state(&self) -> &State {
        self.handler.state()
    }

    /// Returns `NodeHandler`.
    pub fn handler(&self) -> &NodeHandler {
        &self.handler
    }

    /// Returns channel.
    pub fn channel(&self) -> ApiSender {
        ApiSender::new(self.channel.api_requests.0.clone())
    }
}
