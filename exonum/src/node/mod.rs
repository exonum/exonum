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

//! Exonum node that performs consensus algorithm.
//!
//! For details about consensus message handling see messages module documentation.
// spell-checker:ignore cors

pub use self::{
    connect_list::ConnectList, state::{RequestData, State, ValidatorState},
};

// TODO: Temporary solution to get access to WAIT constants. (ECR-167)
pub mod state;

use api::{
    backends::actix::{AllowOrigin, ApiRuntimeConfig, App, AppConfig, Cors, SystemRuntimeConfig},
    ApiAccess, ApiAggregator,
};
use failure;
use futures::{sync::mpsc, Future, Sink};
use tokio_core::reactor::Core;
use toml::Value;

use std::{
    collections::{BTreeMap, HashSet}, fmt, io, net::{SocketAddr, ToSocketAddrs}, sync::Arc, thread,
    time::{Duration, SystemTime},
};

use serde::de::{self, Deserialize, Deserializer};

use blockchain::{
    Blockchain, GenesisConfig, Schema, Service, SharedNodeState, Transaction, ValidatorKeys,
};
use crypto::{self, CryptoHash, Hash, PublicKey, SecretKey};
use events::{
    error::{into_other, log_error, other_error, LogError}, noise::HandshakeParams, HandlerPart,
    InternalEvent, InternalPart, InternalRequest, NetworkConfiguration, NetworkEvent, NetworkPart,
    NetworkRequest, SyncSender, TimeoutRequest,
};
use helpers::{
    config::ConfigManager, fabric::NodePublicConfig, user_agent, Height, Milliseconds, Round,
    ValidatorId,
};
use messages::{Connect, Message, RawMessage};
use storage::{Database, DbOptions};

mod basic;
mod connect_list;
mod consensus;
mod events;
mod requests;

/// External messages.
#[derive(Debug)]
pub enum ExternalMessage {
    /// Add a new connection.
    PeerAdd(ConnectInfo),
    /// Transaction that implements the `Transaction` trait.
    Transaction(Box<dyn Transaction>),
    /// Enable or disable the node.
    Enable(bool),
    /// Shutdown the node.
    Shutdown,
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
pub trait SystemStateProvider: ::std::fmt::Debug + Send + 'static {
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
    /// Shared api state.
    pub api_state: SharedNodeState,
    /// System state.
    pub system_state: Box<dyn SystemStateProvider>,
    /// Channel for messages and timeouts.
    pub channel: NodeSender,
    /// Blockchain.
    pub blockchain: Blockchain,
    /// Known peer addresses.
    pub peer_discovery: Vec<SocketAddr>,
    /// Does this node participate in the consensus?
    is_enabled: bool,
    /// Node role.
    node_role: NodeRole,
    /// Configuration file manager.
    config_manager: Option<ConfigManager>,
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
    /// ConnectList.
    pub connect_list: ConnectList,
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
    /// Cross-origin resource sharing ([CORS][cors]) options for responses returned
    /// by public API handlers.
    ///
    /// [cors]: https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS
    pub public_allow_origin: Option<AllowOrigin>,
    /// Cross-origin resource sharing ([CORS][cors]) options for responses returned
    /// by private API handlers.
    ///
    /// [cors]: https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS
    pub private_allow_origin: Option<AllowOrigin>,
}

impl Default for NodeApiConfig {
    fn default() -> Self {
        Self {
            state_update_timeout: 10_000,
            enable_blockchain_explorer: true,
            public_api_address: None,
            private_api_address: None,
            public_allow_origin: None,
            private_allow_origin: None,
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
    /// Maximum number of queued internal events.
    pub internal_events_capacity: usize,
    /// Maximum number of queued requests from api.
    pub api_requests_capacity: usize,
}

impl Default for EventsPoolCapacity {
    fn default() -> Self {
        Self {
            network_requests_capacity: 512,
            network_events_capacity: 512,
            internal_events_capacity: 128,
            api_requests_capacity: 1024,
        }
    }
}

/// Memory pool configuration parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryPoolConfig {
    /// Maximum number of uncommitted transactions.
    pub tx_pool_capacity: usize,
    /// Sets the maximum number of messages that can be buffered on the event loop's
    /// notification channel before a send will fail.
    pub events_pool_capacity: EventsPoolCapacity,
}

impl Default for MemoryPoolConfig {
    fn default() -> Self {
        Self {
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
    #[serde(deserialize_with = "deserialize_socket_address")]
    pub external_address: SocketAddr,
    /// Network configuration.
    pub network: NetworkConfiguration,
    /// Consensus public key.
    pub consensus_public_key: PublicKey,
    /// Consensus secret key.
    pub consensus_secret_key: SecretKey,
    /// Service public key.
    pub service_public_key: PublicKey,
    /// Service secret key.
    pub service_secret_key: SecretKey,
    /// Api configuration.
    pub api: NodeApiConfig,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
    /// Additional config, usable for services.
    #[serde(default)]
    pub services_configs: BTreeMap<String, Value>,
    /// Optional database configuration.
    #[serde(default)]
    pub database: DbOptions,
    /// Node's ConnectList.
    pub connect_list: ConnectListConfig,
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

/// Channel for messages, timeouts and api requests.
#[derive(Debug)]
pub struct NodeSender {
    /// Internal requests sender.
    pub internal_requests: SyncSender<InternalRequest>,
    /// Network requests sender.
    pub network_requests: SyncSender<NetworkRequest>,
    /// Api requests sender.
    pub api_requests: SyncSender<ExternalMessage>,
}

/// Node role.
#[derive(Debug, Clone, Copy)]
pub enum NodeRole {
    /// Validator node.
    Validator(ValidatorId),
    /// Auditor node.
    Auditor,
}

impl Default for NodeRole {
    fn default() -> Self {
        NodeRole::Auditor
    }
}

impl NodeRole {
    /// Constructs new NodeRole from `validator_id`.
    pub fn new(validator_id: Option<ValidatorId>) -> Self {
        match validator_id {
            Some(validator_id) => NodeRole::Validator(validator_id),
            None => NodeRole::Auditor,
        }
    }

    /// Checks if node is validator.
    pub fn is_validator(self) -> bool {
        match self {
            NodeRole::Validator(_) => true,
            _ => false,
        }
    }

    /// Checks if node is auditor.
    pub fn is_auditor(self) -> bool {
        match self {
            NodeRole::Auditor => true,
            _ => false,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
/// ConnectList representation in node's config file.
pub struct ConnectListConfig {
    /// Peers to which we can connect.
    pub peers: Vec<ConnectInfo>,
}

impl ConnectListConfig {
    /// Creates `ConnectListConfig` from validators public configs.
    pub fn from_node_config(list: &[NodePublicConfig]) -> Self {
        let peers = list.iter()
            .map(|config| ConnectInfo {
                public_key: config.validator_keys.consensus_key,
                address: config.address,
            })
            .collect();

        ConnectListConfig { peers }
    }

    /// Creates `ConnectListConfig` from validators keys and corresponding IP addresses.
    pub fn from_validator_keys(validators_keys: &[ValidatorKeys], peers: &[SocketAddr]) -> Self {
        let peers = peers
            .iter()
            .zip(validators_keys.iter())
            .map(|(a, v)| ConnectInfo {
                address: *a,
                public_key: v.consensus_key,
            })
            .collect();

        ConnectListConfig { peers }
    }

    /// Creates `ConnectListConfig` from `ConnectList`.
    pub fn from_connect_list(connect_list: &ConnectList) -> Self {
        let peers = connect_list
            .peers
            .iter()
            .map(|(pk, a)| ConnectInfo {
                address: *a,
                public_key: *pk,
            })
            .collect();

        ConnectListConfig { peers }
    }

    /// `ConnectListConfig` peers addresses.
    pub fn addresses(&self) -> Vec<SocketAddr> {
        self.peers.iter().map(|p| p.address).collect()
    }
}

impl NodeHandler {
    /// Creates `NodeHandler` using specified `Configuration`.
    pub fn new(
        blockchain: Blockchain,
        external_address: SocketAddr,
        sender: NodeSender,
        system_state: Box<dyn SystemStateProvider>,
        config: Configuration,
        api_state: SharedNodeState,
        config_file_path: Option<String>,
    ) -> Self {
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
            .position(|pk| pk.consensus_key == config.listener.consensus_public_key)
            .map(|id| ValidatorId(id as u16));
        info!("Validator id = '{:?}'", validator_id);
        let connect = Connect::new(
            &config.listener.consensus_public_key,
            external_address,
            system_state.current_time().into(),
            &user_agent::get(),
            &config.listener.consensus_secret_key,
        );

        let connect_list = config.listener.connect_list;
        let state = State::new(
            validator_id,
            config.listener.consensus_public_key,
            config.listener.consensus_secret_key,
            config.service.service_public_key,
            config.service.service_secret_key,
            config.mempool.tx_pool_capacity,
            connect_list,
            stored,
            connect,
            blockchain.get_saved_peers(),
            last_hash,
            last_height,
            system_state.current_time(),
        );

        let node_role = NodeRole::new(validator_id);
        let is_enabled = api_state.is_enabled();
        api_state.set_node_role(node_role);

        let config_manager = match config_file_path {
            Some(path) => Some(ConfigManager::new(path)),
            None => None,
        };

        Self {
            blockchain,
            api_state,
            system_state,
            state,
            channel: sender,
            peer_discovery: config.peer_discovery,
            is_enabled,
            node_role,
            config_manager,
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

    /// Returns value of the minimal propose timeout.
    pub fn min_propose_timeout(&self) -> Milliseconds {
        self.state().consensus_config().min_propose_timeout
    }

    /// Returns value of the maximal propose timeout.
    pub fn max_propose_timeout(&self) -> Milliseconds {
        self.state().consensus_config().max_propose_timeout
    }

    /// Returns threshold starting from which the minimal propose timeout value is used.
    pub fn propose_timeout_threshold(&self) -> u32 {
        self.state().consensus_config().propose_timeout_threshold
    }

    /// Returns `State` of the node.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Performs node initialization, so it starts consensus process from the first round.
    pub fn initialize(&mut self) {
        let listen_address = self.system_state.listen_address();
        info!("Start listening address={}", listen_address);

        let peers: HashSet<_> = {
            let it = self.state.peers().values().map(Connect::addr);
            let it = it.chain(self.peer_discovery.iter().cloned());
            let it = it.filter(|&address| address != listen_address);
            it.collect()
        };

        for address in &peers {
            self.connect(address);
            info!("Trying to connect with peer {}", address);
        }

        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);

        // Recover previous saved round if any
        let round = schema.consensus_round();
        self.state.jump_round(round);
        info!("Jump to round {}", round);

        self.add_round_timeout();
        self.add_status_timeout();
        self.add_peer_exchange_timeout();
        self.add_update_api_state_timeout();

        // Recover cached consensus messages if any. We do this after main initialization and before
        // the start of event processing.
        let messages = schema.consensus_messages_cache();
        for msg in messages.iter() {
            self.handle_message(msg);
        }
    }

    /// Sends the given message to a peer by its id.
    pub fn send_to_validator(&mut self, id: u32, message: &RawMessage) {
        if id as usize >= self.state.validators().len() {
            error!("Invalid validator id: {}", id);
        } else {
            let public_key = self.state.validators()[id as usize].consensus_key;
            self.send_to_peer(public_key, message);
        }
    }

    /// Sends the given message to a peer by its public key.
    pub fn send_to_peer(&mut self, public_key: PublicKey, message: &RawMessage) {
        let address = {
            if let Some(conn) = self.state.peers().get(&public_key) {
                conn.addr()
            } else {
                warn!(
                    "Attempt to send message to peer with key {:?} without connection",
                    public_key
                );
                return;
            }
        };

        self.send_to_addr(&address, message);
    }

    /// Sends `RawMessage` to the specified address.
    pub fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        let public_key = self.state.connect_list().find_key_by_address(&address);

        if let Some(public_key) = public_key {
            trace!("Send to address: {}", address);
            let request = NetworkRequest::SendMessage(*address, message.clone(), *public_key);
            self.channel.network_requests.send(request).log_error();
        } else {
            warn!(
                "Attempt to connect to the peer with address {:?} which \
                 is not in the ConnectList",
                address
            );
        }
    }

    /// Broadcasts given message to all peers.
    pub fn broadcast(&mut self, message: &RawMessage) {
        let peers: Vec<SocketAddr> = self.state
            .peers()
            .values()
            .map(|conn| conn.addr())
            .collect();

        for address in peers {
            self.send_to_addr(&address, message);
        }
    }

    /// Performs connection to the specified network address.
    pub fn connect(&mut self, address: &SocketAddr) {
        let connect = self.state.our_connect_message().clone();

        if self.state.connect_list().is_address_allowed(&address) {
            self.send_to_addr(address, connect.raw());
        } else {
            warn!(
                "Attempt to connect to the peer {:?} which \
                 is not in the ConnectList",
                address
            );
        }
    }

    /// Add timeout request.
    pub fn add_timeout(&mut self, timeout: NodeTimeout, time: SystemTime) {
        let request = TimeoutRequest(time, timeout);
        self.channel
            .internal_requests
            .send(request.into())
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
    pub fn add_round_timeout(&mut self) {
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
    pub fn add_propose_timeout(&mut self) {
        let snapshot = self.blockchain.snapshot();
        let timeout = if Schema::new(&snapshot).transactions_pool_len()
            >= self.propose_timeout_threshold() as usize
        {
            self.min_propose_timeout()
        } else {
            self.max_propose_timeout()
        };

        let time = self.round_start_time(self.state.round()) + Duration::from_millis(timeout);

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
    pub fn add_status_timeout(&mut self) {
        let time = self.system_state.current_time() + Duration::from_millis(self.status_timeout());
        let height = self.state.height();
        self.add_timeout(NodeTimeout::Status(height), time);
    }

    /// Adds `NodeTimeout::Request` timeout with `RequestData` to the channel.
    pub fn add_request_timeout(&mut self, data: RequestData, peer: Option<PublicKey>) {
        trace!("ADD REQUEST TIMEOUT");
        let time = self.system_state.current_time() + data.timeout();
        self.add_timeout(NodeTimeout::Request(data, peer), time);
    }

    /// Adds `NodeTimeout::PeerExchange` timeout to the channel.
    pub fn add_peer_exchange_timeout(&mut self) {
        trace!("ADD PEER EXCHANGE TIMEOUT");
        let time = self.system_state.current_time() + Duration::from_millis(self.peers_timeout());
        self.add_timeout(NodeTimeout::PeerExchange, time);
    }

    /// Adds `NodeTimeout::UpdateApiState` timeout to the channel.
    pub fn add_update_api_state_timeout(&mut self) {
        let time = self.system_state.current_time()
            + Duration::from_millis(self.api_state().state_update_timeout());
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
            self.blockchain, self.peer_discovery
        )
    }
}

/// `TransactionSend` represents interface for sending transactions. For details see `ApiSender`
/// implementation.
pub trait TransactionSend: Send + Sync {
    /// Sends transaction. This can include transaction verification.
    fn send(&self, tx: Box<dyn Transaction>) -> io::Result<()>;
}

impl ApiSender {
    /// Creates new `ApiSender` with given channel.
    pub fn new(inner: mpsc::Sender<ExternalMessage>) -> Self {
        ApiSender(inner)
    }

    /// Add peer to peer list
    pub fn peer_add(&self, addr: ConnectInfo) -> io::Result<()> {
        let msg = ExternalMessage::PeerAdd(addr);
        self.send_external_message(msg)
    }

    /// Sends an external message.
    pub fn send_external_message(&self, message: ExternalMessage) -> io::Result<()> {
        self.0
            .clone()
            .send(message)
            .wait()
            .map(drop)
            .map_err(into_other)
    }
}

impl TransactionSend for ApiSender {
    fn send(&self, tx: Box<dyn Transaction>) -> io::Result<()> {
        if !tx.verify() {
            let msg = "Unable to verify transaction";
            return Err(io::Error::new(io::ErrorKind::Other, msg));
        }
        let msg = ExternalMessage::Transaction(tx);
        self.send_external_message(msg)
    }
}

impl fmt::Debug for ApiSender {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("ApiSender { .. }")
    }
}

fn deserialize_socket_address<'de, D>(value: D) -> Result<SocketAddr, D::Error>
where
    D: Deserializer<'de>,
{
    let address_str: String = Deserialize::deserialize(value)?;
    address_str
        .to_socket_addrs()
        .map_err(de::Error::custom)?
        .next()
        .ok_or_else(|| {
            de::Error::custom(&format!(
                "no one ip belongs to the hostname: {}",
                address_str
            ))
        })
}

/// Data needed to add peer into `ConnectList`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ConnectInfo {
    /// Peer address.
    #[serde(deserialize_with = "deserialize_socket_address")]
    pub address: SocketAddr,
    /// Peer public key.
    pub public_key: PublicKey,
}

impl fmt::Display for ConnectInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.address)
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
    pub internal_requests: (
        mpsc::Sender<InternalRequest>,
        mpsc::Receiver<InternalRequest>,
    ),
    /// Channel for api requests.
    pub api_requests: (
        mpsc::Sender<ExternalMessage>,
        mpsc::Receiver<ExternalMessage>,
    ),
    /// Channel for network events.
    pub network_events: (mpsc::Sender<NetworkEvent>, mpsc::Receiver<NetworkEvent>),
    /// Channel for internal events.
    pub internal_events: (mpsc::Sender<InternalEvent>, mpsc::Receiver<InternalEvent>),
}

/// Node that contains handler (`NodeHandler`) and `NodeApiConfig`.
#[derive(Debug)]
pub struct Node {
    api_options: NodeApiConfig,
    network_config: NetworkConfiguration,
    handler: NodeHandler,
    channel: NodeChannel,
    max_message_len: u32,
}

impl NodeChannel {
    /// Creates `NodeChannel` with the given pool capacities.
    pub fn new(buffer_sizes: &EventsPoolCapacity) -> Self {
        Self {
            network_requests: mpsc::channel(buffer_sizes.network_requests_capacity),
            internal_requests: mpsc::channel(buffer_sizes.internal_events_capacity),
            api_requests: mpsc::channel(buffer_sizes.api_requests_capacity),
            network_events: mpsc::channel(buffer_sizes.network_events_capacity),
            internal_events: mpsc::channel(buffer_sizes.internal_events_capacity),
        }
    }

    /// Returns the channel for sending timeouts, networks and api requests.
    pub fn node_sender(&self) -> NodeSender {
        NodeSender {
            internal_requests: self.internal_requests.0.clone().wait(),
            network_requests: self.network_requests.0.clone().wait(),
            api_requests: self.api_requests.0.clone().wait(),
        }
    }
}

impl Node {
    /// Creates node for the given services and node configuration.
    pub fn new<D: Into<Arc<dyn Database>>>(
        db: D,
        services: Vec<Box<dyn Service>>,
        node_cfg: NodeConfig,
        config_file_path: Option<String>,
    ) -> Self {
        crypto::init();

        let channel = NodeChannel::new(&node_cfg.mempool.events_pool_capacity);
        let mut blockchain = Blockchain::new(
            db,
            services,
            node_cfg.service_public_key,
            node_cfg.service_secret_key.clone(),
            ApiSender::new(channel.api_requests.0.clone()),
        );
        blockchain.initialize(node_cfg.genesis.clone()).unwrap();

        let peers = node_cfg.connect_list.addresses();

        let config = Configuration {
            listener: ListenerConfig {
                consensus_public_key: node_cfg.consensus_public_key,
                consensus_secret_key: node_cfg.consensus_secret_key,
                connect_list: ConnectList::from_config(node_cfg.connect_list),
                address: node_cfg.listen_address,
            },
            service: ServiceConfig {
                service_public_key: node_cfg.service_public_key,
                service_secret_key: node_cfg.service_secret_key,
            },
            mempool: node_cfg.mempool,
            network: node_cfg.network,
            peer_discovery: peers,
        };

        let api_state = SharedNodeState::new(node_cfg.api.state_update_timeout as u64);
        let system_state = Box::new(DefaultSystemState(node_cfg.listen_address));
        let network_config = config.network;
        let handler = NodeHandler::new(
            blockchain,
            node_cfg.external_address,
            channel.node_sender(),
            system_state,
            config,
            api_state,
            config_file_path,
        );
        Self {
            api_options: node_cfg.api,
            handler,
            channel,
            network_config,
            max_message_len: node_cfg.genesis.consensus.max_message_len,
        }
    }

    /// Launches only consensus messages handler.
    /// This may be used if you want to customize api with the `ApiContext`.
    pub fn run_handler(mut self, handshake_params: &HandshakeParams) -> io::Result<()> {
        self.handler.initialize();

        let (handler_part, network_part, timeouts_part) = self.into_reactor();
        let handshake_params = handshake_params.clone();

        let network_thread = thread::spawn(move || {
            let mut core = Core::new()?;
            let handle = core.handle();
            core.handle()
                .spawn(timeouts_part.run(handle).map_err(log_error));
            let network_handler = network_part.run(&core.handle(), &handshake_params);
            core.run(network_handler).map(drop).map_err(|e| {
                other_error(&format!("An error in the `Network` thread occurred: {}", e))
            })
        });

        let mut core = Core::new()?;
        core.run(handler_part.run())
            .map_err(|_| other_error("An error in the `Handler` thread occurred"))?;
        network_thread.join().unwrap()
    }

    /// A generic implementation that launches `Node` and optionally creates threads
    /// for public and private api handlers.
    /// Explorer api prefix is `/api/explorer`
    /// Public api prefix is `/api/services/{service_name}`
    /// Private api prefix is `/api/services/{service_name}`
    pub fn run(self) -> Result<(), failure::Error> {
        // Runs actix-web api.
        let actix_api_runtime = SystemRuntimeConfig {
            api_runtimes: {
                fn into_app_config(allow_origin: AllowOrigin) -> AppConfig {
                    let app_config = move |app: App| -> App {
                        let cors = Cors::from(allow_origin.clone());
                        app.middleware(cors)
                    };
                    Arc::new(app_config)
                };

                let public_api_handler = self.api_options
                    .public_api_address
                    .map(|listen_address| ApiRuntimeConfig {
                        listen_address,
                        access: ApiAccess::Public,
                        app_config: self.api_options
                            .public_allow_origin
                            .clone()
                            .map(into_app_config),
                    })
                    .into_iter();
                let private_api_handler = self.api_options
                    .private_api_address
                    .map(|listen_address| ApiRuntimeConfig {
                        listen_address,
                        access: ApiAccess::Private,
                        app_config: self.api_options
                            .private_allow_origin
                            .clone()
                            .map(into_app_config),
                    })
                    .into_iter();
                // Collects API handlers.
                public_api_handler
                    .chain(private_api_handler)
                    .collect::<Vec<_>>()
            },
            api_aggregator: ApiAggregator::new(
                self.handler.blockchain.clone(),
                self.handler.api_state.clone(),
            ),
        }.start()?;

        // Runs NodeHandler.
        let handshake_params = HandshakeParams::new(
            *self.state().consensus_public_key(),
            self.state().consensus_secret_key().clone(),
            self.max_message_len,
        );
        self.run_handler(&handshake_params)?;

        // Stops actix web runtime.
        actix_api_runtime.stop()?;

        info!("Exonum node stopped");
        Ok(())
    }

    fn into_reactor(self) -> (HandlerPart<NodeHandler>, NetworkPart, InternalPart) {
        let connect_message = self.state().our_connect_message().clone();
        let (network_tx, network_rx) = self.channel.network_events;
        let internal_requests_rx = self.channel.internal_requests.1;
        let network_part = NetworkPart {
            our_connect_message: connect_message,
            listen_address: self.handler.system_state.listen_address(),
            network_requests: self.channel.network_requests,
            network_tx,
            network_config: self.network_config,
            max_message_len: self.max_message_len,
        };

        let (internal_tx, internal_rx) = self.channel.internal_events;
        let handler_part = HandlerPart {
            handler: self.handler,
            internal_rx,
            network_rx,
            api_rx: self.channel.api_requests.1,
        };

        let timeouts_part = InternalPart {
            internal_tx,
            internal_requests_rx,
        };
        (handler_part, network_part, timeouts_part)
    }

    /// Returns `Blockchain` instance.
    pub fn blockchain(&self) -> Blockchain {
        self.handler.blockchain.clone()
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

#[cfg(test)]
mod tests {
    use super::*;
    use blockchain::{ExecutionResult, Schema, Transaction};
    use crypto::gen_keypair;
    use events::EventHandler;
    use helpers;
    use storage::{Database, Fork, MemoryDB};

    messages! {
        const SERVICE_ID = 0;

        struct TxSimple {
            public_key: &PublicKey,
            msg: &str,
        }
    }

    impl Transaction for TxSimple {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, _view: &mut Fork) -> ExecutionResult {
            Ok(())
        }
    }

    #[test]
    fn test_duplicated_transaction() {
        let (p_key, s_key) = gen_keypair();

        let db = Arc::from(Box::new(MemoryDB::new()) as Box<dyn Database>) as Arc<dyn Database>;
        let services = vec![];
        let node_cfg = helpers::generate_testnet_config(1, 16_500)[0].clone();

        let mut node = Node::new(db, services, node_cfg, None);

        let tx = TxSimple::new(&p_key, "Hello, World!", &s_key);

        // Create original transaction.
        let tx_orig = Box::new(tx.clone());
        let event = ExternalMessage::Transaction(tx_orig);
        node.handler.handle_event(event.into());

        // Initial transaction should be added to the pool.
        let snapshot = node.blockchain().snapshot();
        let schema = Schema::new(&snapshot);
        assert_eq!(schema.transactions_pool_len(), 1);

        // Create duplicated transaction.
        let tx_copy = Box::new(tx.clone());
        let event = ExternalMessage::Transaction(tx_copy);
        node.handler.handle_event(event.into());

        // Duplicated transaction shouldn't be added to the pool.
        let snapshot = node.blockchain().snapshot();
        let schema = Schema::new(&snapshot);
        assert_eq!(schema.transactions_pool_len(), 1);
    }
}
