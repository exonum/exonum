// Copyright 2019 The Exonum Team
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
    connect_list::{ConnectList, PeerAddress},
    state::{RequestData, State, ValidatorState},
};

// TODO: Temporary solution to get access to WAIT constants. (ECR-167)
/// Node timeout constants.
pub mod constants {
    pub use super::state::{
        BLOCK_REQUEST_TIMEOUT, PREVOTES_REQUEST_TIMEOUT, PROPOSE_REQUEST_TIMEOUT,
        TRANSACTIONS_REQUEST_TIMEOUT,
    };
}

pub(crate) use self::state::SharedConnectList;

use exonum_keys::Keys;
use exonum_merkledb::{Database, DbOptions, ObjectHash};
use failure::Error;
use futures::{sync::mpsc, Future, Sink};
use tokio_core::reactor::Core;
use tokio_threadpool::Builder as ThreadPoolBuilder;
use toml::Value;

use std::{
    collections::{BTreeMap, HashSet},
    convert::TryFrom,
    fmt,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, SystemTime},
};

use crate::{
    api::{
        backends::actix::{
            AllowOrigin, ApiRuntimeConfig, App, AppConfig, Cors, SystemRuntimeConfig,
        },
        manager::UpdateEndpoints,
        node::SharedNodeState,
        ApiAccess, ApiAggregator,
    },
    blockchain::{
        config::GenesisConfig, Blockchain, BlockchainBuilder, BlockchainMut, ConsensusConfig,
        Schema, ValidatorKeys,
    },
    crypto::{self, Hash, PublicKey, SecretKey},
    events::{
        error::{into_failure, LogError},
        noise::HandshakeParams,
        EventHandler, HandlerPart, InternalEvent, InternalPart, InternalRequest,
        NetworkConfiguration, NetworkEvent, NetworkPart, NetworkRequest, SyncSender,
        TimeoutRequest,
    },
    helpers::{
        config::ConfigManager, user_agent, Height, Milliseconds, Round, ValidateInput, ValidatorId,
    },
    messages::{AnyTx, Connect, ExonumMessage, SignedMessage, Verified},
    runtime::{
        rust::{RustRuntime, ServiceFactory},
        RuntimeInstance,
    },
};

mod basic;
mod connect_list;
mod consensus;
mod events;
mod requests;
mod state;

/// External messages.
#[derive(Debug)]
pub enum ExternalMessage {
    /// Add a new connection.
    PeerAdd(ConnectInfo),
    /// Transaction that implements the `Transaction` trait.
    Transaction(Verified<AnyTx>),
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
pub trait SystemStateProvider: std::fmt::Debug + Send + 'static {
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
    pub blockchain: BlockchainMut,
    /// Known peer addresses.
    pub peer_discovery: Vec<String>,
    /// Does this node participate in the consensus?
    is_enabled: bool,
    /// Node role.
    node_role: NodeRole,
    /// Configuration file manager.
    config_manager: Option<Box<dyn ConfigManager>>,
    /// Can we speed up Propose with transaction pressure?
    allow_expedited_propose: bool,
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
    /// ConnectList.
    pub connect_list: ConnectList,
    /// Socket address.
    pub address: SocketAddr,
}

/// An api configuration options.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NodeApiConfig {
    /// Timeout to update api state.
    pub state_update_timeout: usize,
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
            public_api_address: None,
            private_api_address: None,
            public_allow_origin: None,
            private_allow_origin: None,
        }
    }
}

/// Events pool capacities.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MemoryPoolConfig {
    /// Sets the maximum number of messages that can be buffered on the event loop's
    /// notification channel before a send will fail.
    pub events_pool_capacity: EventsPoolCapacity,
}

impl Default for MemoryPoolConfig {
    fn default() -> Self {
        Self {
            events_pool_capacity: EventsPoolCapacity::default(),
        }
    }
}

/// Configuration for the `Node`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NodeConfig {
    /// Initial consensus configuration that will be written in the genesis block.
    pub consensus: ConsensusConfig,
    /// Network listening address.
    pub listen_address: SocketAddr,
    /// Remote Network address used by this node.
    pub external_address: String,
    /// Network configuration.
    pub network: NetworkConfiguration,
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
    /// Transaction Verification Thread Pool size.
    pub thread_pool_size: Option<u8>,
    /// Path to the master key file.
    pub master_key_path: PathBuf,
    /// Validator keys.
    #[serde(skip)]
    pub keys: Keys,
}

impl NodeConfig {
    /// Returns a service key pair of the node.
    pub fn service_keypair(&self) -> (PublicKey, SecretKey) {
        (self.keys.service_pk(), self.keys.service_sk().clone())
    }
}

impl ValidateInput for NodeConfig {
    type Error = failure::Error;

    fn validate(&self) -> Result<(), Self::Error> {
        let capacity = &self.mempool.events_pool_capacity;
        ensure!(
            capacity.internal_events_capacity > 3,
            "internal_events_capacity({}) must be strictly larger than 2",
            capacity.internal_events_capacity
        );
        ensure!(
            capacity.network_requests_capacity > 0,
            "network_requests_capacity({}) must be strictly larger than 0",
            capacity.network_requests_capacity
        );

        let backend_config = &self.network.http_backend_config;
        ensure!(
            backend_config.server_restart_max_retries > 0,
            "server_restart_max_retries({}) must be strictly larger than 0",
            backend_config.server_restart_max_retries
        );
        ensure!(
            backend_config.server_restart_retry_timeout > 0,
            "server_restart_retry_timeout({}) must be strictly larger than 0",
            backend_config.server_restart_retry_timeout
        );

        // Sanity checks for cases of accidental negative overflows.
        let sanity_max = 2_usize.pow(16);
        ensure!(
            capacity.internal_events_capacity < sanity_max,
            "internal_events_capacity({}) must be smaller than {}",
            capacity.internal_events_capacity,
            sanity_max,
        );
        ensure!(
            capacity.network_requests_capacity < sanity_max,
            "network_requests_capacity({}) must be smaller than {}",
            capacity.network_requests_capacity,
            sanity_max,
        );
        self.consensus.validate()
    }
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
    pub peer_discovery: Vec<String>,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
    /// Validator keys.
    pub keys: Keys,
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
    /// Creates `ConnectListConfig` from validators keys and corresponding IP addresses.
    pub fn from_validator_keys(validators_keys: &[ValidatorKeys], peers: &[String]) -> Self {
        let peers = peers
            .iter()
            .zip(validators_keys.iter())
            .map(|(a, v)| ConnectInfo {
                address: a.clone(),
                public_key: v.consensus_key,
            })
            .collect();

        ConnectListConfig { peers }
    }

    /// Creates `ConnectListConfig` from `ConnectList`.
    pub fn from_connect_list(connect_list: &SharedConnectList) -> Self {
        ConnectListConfig {
            peers: connect_list.peers(),
        }
    }

    /// `ConnectListConfig` peers addresses.
    pub fn addresses(&self) -> Vec<String> {
        self.peers.iter().map(|p| p.address.clone()).collect()
    }
}

impl NodeHandler {
    /// Creates `NodeHandler` using specified `Configuration`.
    pub fn new(
        blockchain: BlockchainMut,
        external_address: &str,
        sender: NodeSender,
        system_state: Box<dyn SystemStateProvider>,
        config: Configuration,
        api_state: SharedNodeState,
        config_manager: Option<Box<dyn ConfigManager>>,
    ) -> Self {
        let (last_hash, last_height) = {
            let block = blockchain.as_ref().last_block();
            (block.object_hash(), block.height.next())
        };

        let snapshot = blockchain.snapshot();
        let consensus_config = Schema::new(&snapshot).consensus_config();
        info!("Creating a node with config: {:#?}", consensus_config);

        let validator_id = consensus_config
            .validator_keys
            .iter()
            .position(|pk| pk.consensus_key == config.keys.consensus_pk())
            .map(|id| ValidatorId(id as u16));
        info!("Validator id = '{:?}'", validator_id);
        let connect = Verified::from_value(
            Connect::new(
                external_address,
                system_state.current_time().into(),
                &user_agent(),
            ),
            config.keys.consensus_pk(),
            &config.keys.consensus_sk(),
        );

        let connect_list = config.listener.connect_list;
        let state = State::new(
            validator_id,
            connect_list,
            consensus_config,
            connect,
            blockchain.as_ref().get_saved_peers(),
            last_hash,
            last_height,
            system_state.current_time(),
            config.keys,
        );

        let node_role = NodeRole::new(validator_id);
        let is_enabled = api_state.is_enabled();
        api_state.set_node_role(node_role);

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
            allow_expedited_propose: true,
        }
    }

    fn sign_message<T>(&self, message: T) -> Verified<T>
    where
        T: TryFrom<SignedMessage> + Into<ExonumMessage> + TryFrom<ExonumMessage>,
    {
        Verified::from_value(
            message,
            self.state.consensus_public_key(),
            self.state.consensus_secret_key(),
        )
    }

    /// Return internal `SharedNodeState`
    pub fn api_state(&self) -> &SharedNodeState {
        &self.api_state
    }

    /// Returns value of the `first_round_timeout` field from the current `ConsensusConfig`.
    pub fn first_round_timeout(&self) -> Milliseconds {
        self.state().consensus_config().first_round_timeout
    }

    /// Returns value of the `round_timeout_increase` field from the current `ConsensusConfig`.
    pub fn round_timeout_increase(&self) -> Milliseconds {
        (self.state().consensus_config().first_round_timeout
            * ConsensusConfig::TIMEOUT_LINEAR_INCREASE_PERCENT)
            / 100
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
            let it = self.state.peers().values().map(Verified::author);
            let it = it.chain(
                self.state()
                    .connect_list()
                    .peers()
                    .into_iter()
                    .map(|i| i.public_key),
            );
            let it = it.filter(|address| address != &self.state.our_connect_message().author());
            it.collect()
        };

        for key in peers {
            self.connect(key);
            info!("Trying to connect with peer {}", key);
        }

        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        // Recover previous saved round if any.
        let round = schema.consensus_round();
        self.state.jump_round(round);
        info!("Jump to round {}", round);

        self.add_timeouts();

        // Recover cached consensus messages if any. We do this after main initialization and before
        // the start of event processing.
        let messages = schema.consensus_messages_cache();
        for msg in messages.iter() {
            self.handle_message(msg);
        }
    }

    /// Runs the node's basic timers.
    fn add_timeouts(&mut self) {
        self.add_round_timeout();
        self.add_status_timeout();
        self.add_peer_exchange_timeout();
        self.add_update_api_state_timeout();
    }

    /// Sends the given message to a peer by its public key.
    pub fn send_to_peer<T: Into<SignedMessage>>(&mut self, public_key: PublicKey, message: T) {
        let message = message.into();
        let request = NetworkRequest::SendMessage(public_key, message);
        self.channel.network_requests.send(request).log_error();
    }

    /// Broadcasts given message to all peers.
    pub(crate) fn broadcast<M: Into<SignedMessage>>(&mut self, message: M) {
        let peers: Vec<PublicKey> = self
            .state
            .peers()
            .iter()
            .filter_map(|(pubkey, _)| {
                if self.state.connect_list().is_peer_allowed(pubkey) {
                    Some(*pubkey)
                } else {
                    None
                }
            })
            .collect();
        let message = message.into();
        for address in peers {
            self.send_to_peer(address, message.clone());
        }
    }

    /// Performs connection to the specified network address.
    pub fn connect(&mut self, key: PublicKey) {
        let connect = self.state.our_connect_message().clone();
        self.send_to_peer(key, connect);
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
        let timeout = if self.need_faster_propose() {
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

    fn maybe_add_propose_timeout(&mut self) {
        if self.allow_expedited_propose && self.need_faster_propose() {
            info!("Add expedited propose timeout");
            self.add_propose_timeout();
            self.allow_expedited_propose = false;
        }
    }

    fn need_faster_propose(&self) -> bool {
        let snapshot = self.blockchain.snapshot();
        let pending_tx_count =
            Schema::new(&snapshot).transactions_pool_len() + self.state.tx_cache_len() as u64;
        pending_tx_count >= u64::from(self.propose_timeout_threshold())
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
        self.blockchain.as_ref().last_block().object_hash()
    }

    /// Returns the number of uncommitted transactions.
    pub fn uncommitted_txs_count(&self) -> u64 {
        self.blockchain.as_ref().pool_size() + self.state.tx_cache_len() as u64
    }

    /// Returns start time of the requested round.
    pub fn round_start_time(&self, round: Round) -> SystemTime {
        // Round start time = H + (r - 1) * t0 + (r-1)(r-2)/2 * dt
        // Where:
        // H - height start time
        // t0 - Round(1) timeout length, dt - timeout increase value
        // r - round number, r = 1,2,...
        let previous_round: u64 = round.previous().into();
        let ms = previous_round * self.first_round_timeout()
            + (previous_round * previous_round.saturating_sub(1)) / 2
                * self.round_timeout_increase();
        self.state.height_start_time() + Duration::from_millis(ms)
    }
}

impl fmt::Debug for NodeHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NodeHandler {{ channel: Channel {{ .. }}, blockchain: {:?}, peer_discovery: {:?} }}",
            self.blockchain, self.peer_discovery
        )
    }
}

impl ApiSender {
    /// Creates new `ApiSender` with given channel.
    pub fn new(inner: mpsc::Sender<ExternalMessage>) -> Self {
        ApiSender(inner)
    }

    /// Creates a dummy sender which cannot send messages.
    pub fn closed() -> Self {
        ApiSender(mpsc::channel(0).0)
    }

    /// Add peer to peer list
    pub fn peer_add(&self, addr: ConnectInfo) -> Result<(), Error> {
        let msg = ExternalMessage::PeerAdd(addr);
        self.send_external_message(msg)
    }

    /// Sends an external message.
    pub fn send_external_message(&self, message: ExternalMessage) -> Result<(), Error> {
        self.0
            .clone()
            .send(message)
            .wait()
            .map(drop)
            .map_err(into_failure)
    }

    /// Broadcast transaction to other nodes in the blockchain network.
    pub fn broadcast_transaction(&self, tx: Verified<AnyTx>) -> Result<(), Error> {
        let msg = ExternalMessage::Transaction(tx);
        self.send_external_message(msg)
    }
}

impl fmt::Debug for ApiSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiSender").finish()
    }
}

/// Data needed to add peer into `ConnectList`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ConnectInfo {
    /// Peer address.
    pub address: String,
    /// Peer public key.
    pub public_key: PublicKey,
}

impl fmt::Display for ConnectInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    /// Channel for transferring API endpoints from producers (e.g., Rust runtime) to the
    /// `ApiManager`.
    pub endpoints: (
        mpsc::Sender<UpdateEndpoints>,
        mpsc::Receiver<UpdateEndpoints>,
    ),
    /// Channel for API requests.
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
    api_runtime_config: SystemRuntimeConfig,
    api_options: NodeApiConfig,
    network_config: NetworkConfiguration,
    handler: NodeHandler,
    channel: NodeChannel,
    max_message_len: u32,
    thread_pool_size: Option<u8>,
}

impl NodeChannel {
    /// Creates `NodeChannel` with the given pool capacities.
    pub fn new(buffer_sizes: &EventsPoolCapacity) -> Self {
        Self {
            network_requests: mpsc::channel(buffer_sizes.network_requests_capacity),
            internal_requests: mpsc::channel(buffer_sizes.internal_events_capacity),
            endpoints: mpsc::channel(buffer_sizes.internal_events_capacity),
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
    pub fn new(
        database: impl Into<Arc<dyn Database>>,
        external_runtimes: impl IntoIterator<Item = impl Into<RuntimeInstance>>,
        services: impl IntoIterator<Item = Box<dyn ServiceFactory>>,
        node_cfg: NodeConfig,
        genesis_config: GenesisConfig,
        config_manager: Option<Box<dyn ConfigManager>>,
    ) -> Self {
        node_cfg
            .validate()
            .expect("Node configuration is inconsistent");
        let channel = NodeChannel::new(&node_cfg.mempool.events_pool_capacity);
        let blockchain = Blockchain::new(
            database,
            node_cfg.service_keypair(),
            ApiSender::new(channel.api_requests.0.clone()),
        );
        let rust_runtime = services.into_iter().fold(
            RustRuntime::new(channel.endpoints.0.clone()),
            |runtime, factory| runtime.with_factory(factory),
        );

        let mut blockchain_builder =
            BlockchainBuilder::new(blockchain, genesis_config).with_runtime(rust_runtime);
        for runtime in external_runtimes {
            blockchain_builder = blockchain_builder.with_runtime(runtime);
        }
        let blockchain = blockchain_builder.build().unwrap_or_else(|err| {
            panic!(
                "Blockchain initialization failed with the following error: {}",
                err
            )
        });

        Self::with_blockchain(blockchain, channel, node_cfg, config_manager)
    }

    /// Creates a node for the given blockchain and node configuration.
    pub fn with_blockchain(
        blockchain: BlockchainMut,
        channel: NodeChannel,
        node_cfg: NodeConfig,
        config_manager: Option<Box<dyn ConfigManager>>,
    ) -> Self {
        crypto::init();

        let peers = node_cfg.connect_list.addresses();
        let config = Configuration {
            listener: ListenerConfig {
                connect_list: ConnectList::from_config(node_cfg.connect_list),
                address: node_cfg.listen_address,
            },
            service: ServiceConfig {
                service_public_key: node_cfg.keys.service_pk(),
                service_secret_key: node_cfg.keys.service_sk().clone(),
            },
            mempool: node_cfg.mempool,
            network: node_cfg.network,
            peer_discovery: peers,
            keys: node_cfg.keys,
        };

        let api_state = SharedNodeState::new(node_cfg.api.state_update_timeout as u64);
        let system_state = Box::new(DefaultSystemState(node_cfg.listen_address));
        let network_config = config.network;

        let api_cfg = node_cfg.api.clone();
        let api_runtime_config = SystemRuntimeConfig {
            api_runtimes: {
                fn into_app_config(allow_origin: AllowOrigin) -> AppConfig {
                    let app_config = move |app: App| -> App {
                        let cors = Cors::from(allow_origin.clone());
                        app.middleware(cors)
                    };
                    Arc::new(app_config)
                };

                let public_api_handler = api_cfg
                    .public_api_address
                    .map(|listen_address| ApiRuntimeConfig {
                        listen_address,
                        access: ApiAccess::Public,
                        app_config: api_cfg.public_allow_origin.clone().map(into_app_config),
                    })
                    .into_iter();
                let private_api_handler = api_cfg
                    .private_api_address
                    .map(|listen_address| ApiRuntimeConfig {
                        listen_address,
                        access: ApiAccess::Private,
                        app_config: api_cfg.private_allow_origin.clone().map(into_app_config),
                    })
                    .into_iter();
                // Collects API handlers.
                public_api_handler
                    .chain(private_api_handler)
                    .collect::<Vec<_>>()
            },
            api_aggregator: ApiAggregator::new(blockchain.immutable_view(), api_state.clone()),
            server_restart_retry_timeout: node_cfg
                .network
                .http_backend_config
                .server_restart_retry_timeout,
            server_restart_max_retries: node_cfg
                .network
                .http_backend_config
                .server_restart_max_retries,
        };

        let handler = NodeHandler::new(
            blockchain,
            &node_cfg.external_address,
            channel.node_sender(),
            system_state,
            config,
            api_state,
            config_manager,
        );

        Self {
            api_options: api_cfg,
            handler,
            channel,
            network_config,
            max_message_len: node_cfg.consensus.max_message_len,
            thread_pool_size: node_cfg.thread_pool_size,
            api_runtime_config,
        }
    }

    /// Launches only consensus messages handler.
    /// This may be used if you want to customize api with the `ApiContext`.
    pub fn run_handler(mut self, handshake_params: &HandshakeParams) -> Result<(), Error> {
        self.handler.initialize();

        let pool_size = self.thread_pool_size;
        let (handler_part, network_part, internal_part) = self.into_reactor();
        let handshake_params = handshake_params.clone();

        let network_thread = thread::spawn(move || {
            let mut core = Core::new().map_err(into_failure)?;
            let handle = core.handle();

            let mut pool_builder = ThreadPoolBuilder::new();
            if let Some(pool_size) = pool_size {
                pool_builder.pool_size(pool_size as usize);
            }
            let thread_pool = pool_builder.build();
            let executor = thread_pool.sender().clone();

            core.handle().spawn(internal_part.run(handle, executor));

            let network_handler = network_part.run(&core.handle(), &handshake_params);
            core.run(network_handler)
                .map(drop)
                .map_err(|e| format_err!("An error in the `Network` thread occurred: {}", e))
        });

        let mut core = Core::new().map_err(into_failure)?;
        core.run(handler_part.run())
            .map_err(|_| format_err!("An error in the `Handler` thread occurred"))?;

        network_thread.join().unwrap()
    }

    /// A generic implementation that launches `Node` and optionally creates threads
    /// for public and private api handlers.
    /// Explorer api prefix is `/api/explorer`
    /// Public api prefix is `/api/services/{service_name}`
    /// Private api prefix is `/api/services/{service_name}`
    pub fn run(self) -> Result<(), failure::Error> {
        trace!("Running node.");

        let api_state = self.handler.api_state.clone();

        // Runs NodeHandler.
        let handshake_params = HandshakeParams::new(
            self.state().consensus_public_key(),
            self.state().consensus_secret_key().clone(),
            self.state().connect_list().clone(),
            self.state().our_connect_message().clone(),
            self.max_message_len,
        );
        self.run_handler(&handshake_params)?;
        // Stop ws server.
        api_state.shutdown_broadcast_server();
        Ok(())
    }

    fn into_reactor(self) -> (HandlerPart<impl EventHandler>, NetworkPart, InternalPart) {
        let connect_message = self.state().our_connect_message().clone();
        let connect_list = self.state().connect_list().clone();

        self.api_runtime_config
            .start(self.channel.endpoints.1)
            .expect("Failed to start api_runtime.");
        let (network_tx, network_rx) = self.channel.network_events;
        let internal_requests_rx = self.channel.internal_requests.1;
        let network_part = NetworkPart {
            our_connect_message: connect_message,
            listen_address: self.handler.system_state.listen_address(),
            network_requests: self.channel.network_requests,
            network_tx,
            network_config: self.network_config,
            max_message_len: self.max_message_len,
            connect_list,
        };

        let (internal_tx, internal_rx) = self.channel.internal_events;
        let handler_part = HandlerPart {
            handler: self.handler,
            internal_rx,
            network_rx,
            api_rx: self.channel.api_requests.1,
        };

        let internal_part = InternalPart {
            internal_tx,
            internal_requests_rx,
        };
        (handler_part, network_part, internal_part)
    }

    /// Returns `Blockchain` instance.
    pub fn blockchain(&self) -> Blockchain {
        self.handler.blockchain.as_ref().clone()
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
    use exonum_merkledb::TemporaryDB;

    use super::*;
    use crate::{blockchain::config::GenesisConfigBuilder, helpers, runtime::RuntimeInstance};

    #[test]
    fn test_good_internal_events_config() {
        let db = Arc::from(Box::new(TemporaryDB::new()) as Box<dyn Database>) as Arc<dyn Database>;
        let services = vec![];
        let external_runtimes: Vec<RuntimeInstance> = vec![];
        let node_cfg = helpers::generate_testnet_config(1, 16_500)[0].clone();
        let genesis_config =
            GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone()).build();
        let _ = Node::new(
            db,
            external_runtimes,
            services,
            node_cfg,
            genesis_config,
            None,
        );
    }

    #[test]
    #[should_panic(expected = "internal_events_capacity(0) must be strictly larger than 2")]
    fn test_bad_internal_events_capacity_too_small() {
        let db = Arc::from(Box::new(TemporaryDB::new()) as Box<dyn Database>) as Arc<dyn Database>;
        let services = vec![];
        let external_runtimes: Vec<RuntimeInstance> = vec![];
        let mut node_cfg = helpers::generate_testnet_config(1, 16_500)[0].clone();
        node_cfg
            .mempool
            .events_pool_capacity
            .internal_events_capacity = 0;
        let genesis_config =
            GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone()).build();
        let _ = Node::new(
            db,
            external_runtimes,
            services,
            node_cfg,
            genesis_config,
            None,
        );
    }

    #[test]
    #[should_panic(expected = "network_requests_capacity(0) must be strictly larger than 0")]
    fn test_bad_network_requests_capacity_too_small() {
        let db = Arc::from(Box::new(TemporaryDB::new()) as Box<dyn Database>) as Arc<dyn Database>;
        let services = vec![];
        let external_runtimes: Vec<RuntimeInstance> = vec![];
        let mut node_cfg = helpers::generate_testnet_config(1, 16_500)[0].clone();
        node_cfg
            .mempool
            .events_pool_capacity
            .network_requests_capacity = 0;
        let genesis_config =
            GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone()).build();
        let _ = Node::new(
            db,
            external_runtimes,
            services,
            node_cfg,
            genesis_config,
            None,
        );
    }

    #[test]
    #[should_panic(expected = "must be smaller than 65536")]
    fn test_bad_internal_events_capacity_too_large() {
        let accidental_large_value = 0_usize.overflowing_sub(1).0;
        let db = Arc::from(Box::new(TemporaryDB::new()) as Box<dyn Database>) as Arc<dyn Database>;

        let external_runtimes: Vec<RuntimeInstance> = vec![];
        let services = vec![];

        let mut node_cfg = helpers::generate_testnet_config(1, 16_500)[0].clone();
        node_cfg
            .mempool
            .events_pool_capacity
            .internal_events_capacity = accidental_large_value;
        let genesis_config =
            GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone()).build();
        let _ = Node::new(
            db,
            external_runtimes,
            services,
            node_cfg,
            genesis_config,
            None,
        );
    }

    #[test]
    #[should_panic(expected = "must be smaller than 65536")]
    fn test_bad_network_requests_capacity_too_large() {
        let accidental_large_value = 0_usize.overflowing_sub(1).0;
        let db = Arc::from(Box::new(TemporaryDB::new()) as Box<dyn Database>) as Arc<dyn Database>;

        let external_runtimes: Vec<RuntimeInstance> = vec![];
        let services = vec![];

        let mut node_cfg = helpers::generate_testnet_config(1, 16_500)[0].clone();
        node_cfg
            .mempool
            .events_pool_capacity
            .network_requests_capacity = accidental_large_value;
        let genesis_config =
            GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone()).build();
        let _ = Node::new(
            db,
            external_runtimes,
            services,
            node_cfg,
            genesis_config,
            None,
        );
    }
}
