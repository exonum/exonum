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
use futures::{Sink, Future};
use futures::sync::mpsc::Sender;
use tokio_core::reactor::Handle;

use std::io;
use std::net::SocketAddr;
use std::time::{SystemTime, Duration};
use std::collections::BTreeMap;
use std::fmt;

use crypto::{PublicKey, SecretKey, Hash};
use events::{Milliseconds};
use blockchain::{SharedNodeState, Blockchain, Schema, GenesisConfig, Transaction};
use messages::{Connect, RawMessage, Message};
use tokio::network::NetworkConfiguration;
use tokio::error::{into_other, forget_result, log_error};
use tokio::handler::{SystemStateProvider, TimeoutRequest};
use tokio::network::NetworkRequest;

pub use self::state::{State, Round, Height, RequestData, ValidatorId, TxPool, ValidatorState};
pub use self::whitelist::Whitelist;
pub use tokio::{Node, NodeChannel, NodeSender};

mod events;
mod basic;
mod consensus;
mod requests;
mod whitelist;
pub mod state; // TODO: temporary solution to get access to WAIT consts
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

/// Transactions sender.
#[derive(Clone)]
pub struct ApiSender(pub Sender<ExternalMessage>);

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
    /// Event loop handle
    pub handle: Handle,
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
    /// Network configuration.
    pub network: NetworkConfiguration,
    /// Known peer addresses.
    pub peer_discovery: Vec<SocketAddr>,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
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
        handle: Handle,
    ) -> Self {
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
            .position(|pk| {
                pk.consensus_key == config.listener.consensus_public_key
            })
            .map(|id| id as ValidatorId);
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
            handle,
        }
    }

    /// Events handle
    pub fn tokio_handle(&self) -> Handle {
        self.handle.clone()
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
        info!(
            "Start listening address={}",
            addr
        );
        for address in &self.peer_discovery.clone() {
            if address == &self.system_state.listen_address() {
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
    pub fn send_to_validator(&self, id: u32, message: &RawMessage) {
        // TODO: check validator id
        let public_key = self.state.validators()[id as usize].consensus_key;
        self.send_to_peer(public_key, message);
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
        let send_future = self.channel.network
            .clone()
            .send(request)
            .map(forget_result)
            .map_err(log_error);
        self.tokio_handle().spawn(send_future);
    }

    /// Broadcasts given message to all peers.
    // TODO: use Into<RawMessage>
    pub fn broadcast(&self, message: &RawMessage) {
        for conn in self.state.peers().values() {
            let addr = conn.addr();
            self.send_to_addr(&addr, message);
        }
    }

    /// Performs connection to the specified network address.
    pub fn connect(&self, address: &SocketAddr) {
        let connect = self.state.our_connect_message().clone();
        self.send_to_addr(address, connect.raw());
    }

    /// Add timeout request.
    pub fn add_timeout(&self, timeout: NodeTimeout, time: SystemTime) {
        let duration = time.duration_since(self.system_state.current_time())
            .unwrap_or_else(|_| Duration::from_millis(0));
        let request = TimeoutRequest(duration, timeout);
        let send_future = self.channel.timeout
            .clone()
            .send(request)
            .map(forget_result)
            .map_err(log_error);
        self.tokio_handle().spawn(send_future);
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
        let time = self.round_start_time(self.state.round() + 1);
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
        let ms = (round - 1) as u64 * self.round_timeout();
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
    pub fn new(inner: Sender<ExternalMessage>) -> ApiSender {
        ApiSender(inner)
    }

    /// Addr peer to peer list
    pub fn peer_add(&self, addr: SocketAddr) -> io::Result<()> {
        let msg = ExternalMessage::PeerAdd(addr);
        self.0.clone().send(msg).wait().map(forget_result).map_err(
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
        self.0.clone().send(msg).wait().map(forget_result).map_err(
            into_other,
        )
    }
}

impl fmt::Debug for ApiSender {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("ApiSender { .. }")
    }
}
