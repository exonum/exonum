// Copyright 2020 The Exonum Team
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

//! Exonum node that handles consensus algorithm, interaction with other nodes and external clients.
//!
//! # Overview
//!
//! This crate contains the following APIs:
//!
//! - [`Node`] encapsulates a full-fledged Exonum node
//! - [`NodeChannel`] and [`ShutdownHandle`] allow to interact with the node
//!   (mind that `NodeChannel` is relatively low-level)
//! - Configuration types, "rooted" in [`NodeConfig`], allow to configure aspects
//!   of the `Node` behavior
//!
//! There are also some types / methods excluded from the docs, but they are hidden for a reason:
//! such APIs are considered an implementation detail and are exempt from semantic versioning.
//! (In other words, these APIs may change or be removed in any release without prior warning.)
//!
//! [`Node`]: struct.Node.html
//! [`NodeChannel`]: struct.NodeChannel.html
//! [`ShutdownHandle`]: struct.ShutdownHandle.html
//! [`NodeConfig`]: struct.NodeConfig.html

// spell-checker:ignore cors

#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    // Next `cast_*` lints don't give alternatives.
    clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss,
    // Next lints produce too much noise/false positives.
    clippy::module_name_repetitions, clippy::similar_names, clippy::must_use_candidate,
    clippy::pub_enum_variant_names,
    // '... may panic' lints.
    clippy::indexing_slicing,
    // Too much work to fix.
    clippy::missing_errors_doc, clippy::missing_const_for_fn
)]

pub use crate::{
    connect_list::{ConnectInfo, ConnectListConfig},
    plugin::{NodePlugin, PluginApiContext, SharedNodeState},
};

use actix_rt::System;
use anyhow::{ensure, format_err};
use exonum::{
    blockchain::{
        config::GenesisConfig, ApiSender, Blockchain, BlockchainBuilder, BlockchainMut,
        ConsensusConfig, Schema, SendError,
    },
    crypto::{self, Hash, PublicKey},
    helpers::{user_agent, Height, Milliseconds, Round, ValidateInput, ValidatorId},
    keys::Keys,
    merkledb::{Database, ObjectHash},
    messages::{AnyTx, IntoMessage, SignedMessage, Verified},
    runtime::RuntimeInstance,
};
use exonum_api::{
    AllowOrigin, ApiAccess, ApiAggregator, ApiManager, ApiManagerConfig, UpdateEndpoints,
    WebServerConfig,
};
use futures::{
    channel::{mpsc, oneshot},
    prelude::*,
};
use log::{info, trace};
use serde_derive::{Deserialize, Serialize};
use tokio::time::delay_for;

use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    fmt, io,
    net::SocketAddr,
    sync::Arc,
    thread,
    time::{Duration, SystemTime},
};

use crate::{
    connect_list::ConnectList,
    events::{
        HandlerPart, HandshakeParams, InternalEvent, InternalPart, InternalRequest, NetworkEvent,
        NetworkPart, NetworkRequest, SyncSender, TimeoutRequest,
    },
    messages::Connect,
    pool::{ManagePool, StandardPoolManager},
    schema::NodeSchema,
    state::{RequestData, State},
};

mod basic;
mod connect_list;
mod consensus;
mod events;
mod events_impl;
pub mod helpers;
mod messages;
mod plugin;
pub mod pool;
mod proto;
mod requests;
#[cfg(test)]
mod sandbox;
mod schema;
mod state;

// Logically private types re-exported for benchmarks.
#[doc(hidden)]
pub mod _bench_types {
    pub use crate::{
        events::{
            Event, EventHandler, EventOutcome, HandlerPart, InternalPart, InternalRequest,
            NetworkEvent,
        },
        messages::Message as PeerMessage,
    };
}

/// External messages sent to the node.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExternalMessage {
    /// Add a new connection.
    PeerAdd(ConnectInfo),
    /// Enable or disable the node.
    Enable(bool),
    /// Shutdown the node.
    Shutdown,
}

/// Node timeout types.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum NodeTimeout {
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
    /// Flush uncommitted transactions into the database.
    FlushPool,
}

/// A helper trait that provides the node with information about the state of the system such
/// as current time or listen address.
pub(crate) trait SystemStateProvider: fmt::Debug + Send + 'static {
    /// Returns the current address that the node listens on.
    fn listen_address(&self) -> SocketAddr;
    /// Return the current system time.
    fn current_time(&self) -> SystemTime;
}

/// Handler responsible for the consensus algorithm.
///
/// # Stability
///
/// This type and its methods are considered an implementation detail of the Exonum node and are
/// thus exempt from semantic versioning.
pub(crate) struct NodeHandler {
    /// Shared API state.
    pub api_state: SharedNodeState,
    /// Blockchain.
    pub blockchain: BlockchainMut,
    /// Node plugins.
    plugins: Vec<Box<dyn NodePlugin>>,
    /// State of the `NodeHandler`.
    state: State,
    /// System state.
    system_state: Box<dyn SystemStateProvider>,
    /// Channel for messages and timeouts.
    channel: NodeSender,
    /// Known peer addresses.
    peer_discovery: Vec<String>,
    /// Does this node participate in the consensus?
    is_enabled: bool,
    /// Node role.
    node_role: NodeRole,
    /// Configuration file manager.
    config_manager: Option<Box<dyn ConfigManager>>,
    /// Can we speed up Propose with transaction pressure?
    allow_expedited_propose: bool,
    /// Pool manager.
    pool_manager: Box<dyn ManagePool>,
}

/// HTTP API configuration options.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NodeApiConfig {
    /// Timeout to update API state.
    pub state_update_timeout: usize,
    /// Listen address for public API endpoints.
    pub public_api_address: Option<SocketAddr>,
    /// Listen address for private API endpoints.
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
    /// HTTP server restart policy. The server is restarted each time the list of endpoints
    /// is updated (e.g., due to a new service initialization).
    #[serde(default)]
    pub server_restart: ServerRestartPolicy,
}

impl Default for NodeApiConfig {
    fn default() -> Self {
        Self {
            state_update_timeout: 10_000,
            public_api_address: None,
            private_api_address: None,
            public_allow_origin: None,
            private_allow_origin: None,
            server_restart: ServerRestartPolicy::default(),
        }
    }
}

/// HTTP server restart policy.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct ServerRestartPolicy {
    /// The number of attempts to restart the HTTP server.
    pub max_retries: u16,
    /// The interval in milliseconds between attempts of restarting the HTTP server.
    pub retry_timeout: u64,
}

impl Default for ServerRestartPolicy {
    fn default() -> Self {
        Self {
            max_retries: 20,
            retry_timeout: 500,
        }
    }
}

/// P2P network configuration of an Exonum node.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NetworkConfiguration {
    /// Maximum number of incoming connections established with peers at any given time.
    pub max_incoming_connections: usize,
    /// Maximum number of outgoing connections established with peers at any given time.
    pub max_outgoing_connections: usize,
    /// Switches on [`TCP_NODELAY`] option.
    ///
    /// [`TCP_NODELAY`]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_nodelay
    pub tcp_nodelay: bool,
    /// Allows to set interval between keep-alive TCP probes. If set to `None`, keep-alive probes
    /// will be disabled.
    pub tcp_keep_alive: Option<u64>,
    /// Retry timeout if an outgoing connection to a peer fails.
    pub tcp_connect_retry_timeout: Milliseconds,
    /// Maximum number of retries when connecting to a peer.
    pub tcp_connect_max_retries: u64,
}

impl Default for NetworkConfiguration {
    fn default() -> Self {
        Self {
            max_incoming_connections: 128,
            max_outgoing_connections: 128,
            tcp_keep_alive: None,
            tcp_nodelay: true,
            tcp_connect_retry_timeout: 15_000,
            tcp_connect_max_retries: 10,
        }
    }
}

/// Events pool capacities.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EventsPoolCapacity {
    /// Maximum number of queued outgoing network messages.
    network_requests_capacity: usize,
    /// Maximum number of queued incoming network messages.
    network_events_capacity: usize,
    /// Maximum number of queued internal events.
    internal_events_capacity: usize,
    /// Maximum number of queued requests from api.
    api_requests_capacity: usize,
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
///
/// # Examples
///
/// For most applications, you should use the value returned by `MemoryPoolConfig::default()`, maybe
/// overwriting some fields afterwards:
///
/// ```
/// # use exonum_node::{FlushPoolStrategy, MemoryPoolConfig};
/// let mut pool_config = MemoryPoolConfig::default();
/// // Increase flush pool interval to 100 milliseconds.
/// pool_config.flush_pool_strategy = FlushPoolStrategy::Timeout {
///     timeout: 100,
/// };
/// // Use the config somewhere...
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MemoryPoolConfig {
    /// Sets the maximum number of messages that can be buffered on the event loop's
    /// notification channel before a send will fail.
    pub events_pool_capacity: EventsPoolCapacity,

    /// Interval in milliseconds between flushing the transaction cache to the persistent pool.
    ///
    /// This value influences how fast transactions appear in the persistent pool after they
    /// have been initially processed by the node. With `Never` setting, transactions will not
    /// be persisted at all before appearing in a block, which may negatively influence
    /// some applications (e.g., the `GET transactions` endpoint of the explorer service).
    /// On the other hand of the spectrum, there is `Immediate`, which flushes each transaction
    /// separately after its initial processing. In the middle, there is `Timeout`, which
    /// allows to specify the coherence interval for the pool.
    #[serde(default)]
    pub flush_pool_strategy: FlushPoolStrategy,
}

/// Strategy to flush transactions into the pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum FlushPoolStrategy {
    /// Never flush the transactions to the persistent pool.
    ///
    /// This setting is best for performance, but may harm the availability of some APIs of the node
    /// (e.g., the `GET transactions` endpoint of the explorer service).
    Never,

    /// Flush transactions on the specified timeout in milliseconds.
    ///
    /// The recommended values are order of 20ms.
    Timeout {
        /// Timeout value in milliseconds.
        timeout: Milliseconds,
    },

    /// Flush each transaction after receiving it.
    ///
    /// Beware that this setting can harm performance of the node under high load.
    Immediate,
}

impl Default for FlushPoolStrategy {
    fn default() -> Self {
        Self::Timeout { timeout: 20 }
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
    /// P2P network configuration.
    pub network: NetworkConfiguration,
    /// HTTP API configuration.
    pub api: NodeApiConfig,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
    /// List of peers the node will connect to on start.
    pub connect_list: ConnectListConfig,
    /// Number of threads allocated for transaction verification.
    pub thread_pool_size: Option<u8>,
}

impl ValidateInput for NodeConfig {
    type Error = anyhow::Error;

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

        let restart_policy = &self.api.server_restart;
        ensure!(
            restart_policy.max_retries > 0,
            "`server_restart.max_retries` must be strictly larger than 0"
        );
        ensure!(
            restart_policy.retry_timeout > 0,
            "`server_restart.retry_timeout` must be strictly larger than 0"
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
///
/// This type is considered an implementation detail of the node handler; it is exempt from
/// semantic versioning.
#[derive(Debug, Clone)]
pub(crate) struct Configuration {
    /// Connection list.
    pub connect_list: ConnectList,
    /// Network configuration.
    pub network: NetworkConfiguration,
    /// Known peer addresses.
    pub peer_discovery: Vec<String>,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
    /// Validator keys.
    pub keys: Keys,
}

/// Channel for messages, timeouts and api requests. Consumed by the `NodeHandler` constructor.
#[derive(Debug)]
pub(crate) struct NodeSender {
    /// Internal requests sender.
    pub internal_requests: SyncSender<InternalRequest>,
    /// Network requests sender.
    pub network_requests: SyncSender<NetworkRequest>,
    /// Transactions sender. This sender is not used by the node, but is necessary to guarantee
    /// that the node won't terminate if external senders are dropped.
    pub _transactions: mpsc::Sender<Verified<AnyTx>>,
    /// API requests sender. This sender is not used by the node, but is necessary to guarantee
    /// that the node won't terminate if external senders are dropped.
    pub _api_requests: mpsc::Sender<ExternalMessage>,
}

/// Node role.
#[derive(Debug, Clone, Copy)]
pub(crate) enum NodeRole {
    /// Validator node.
    Validator(ValidatorId),
    /// Auditor node.
    Auditor,
}

impl Default for NodeRole {
    fn default() -> Self {
        Self::Auditor
    }
}

impl NodeRole {
    /// Constructs new `NodeRole` from `validator_id`.
    pub fn new(validator_id: Option<ValidatorId>) -> Self {
        match validator_id {
            Some(validator_id) => Self::Validator(validator_id),
            None => Self::Auditor,
        }
    }

    /// Checks if node is validator.
    pub fn is_validator(self) -> bool {
        match self {
            Self::Validator(_) => true,
            _ => false,
        }
    }
}

impl NodeHandler {
    /// Creates `NodeHandler` using specified `Configuration`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        blockchain: BlockchainMut,
        external_address: &str,
        sender: NodeSender,
        system_state: Box<dyn SystemStateProvider>,
        config: Configuration,
        api_state: SharedNodeState,
        config_manager: Option<Box<dyn ConfigManager>>,
        pool_manager: Box<dyn ManagePool>,
    ) -> Self {
        let snapshot = blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        let last_block = schema.last_block();
        let last_block_skip = schema.block_skip();
        let consensus_config = schema.consensus_config();
        info!("Creating a node with config: {:#?}", consensus_config);

        let connect = Connect::new(
            external_address,
            system_state.current_time().into(),
            &user_agent(),
        );
        let peers = NodeSchema::new(&blockchain.snapshot())
            .peers_cache()
            .iter()
            .collect();
        let peer_discovery = config.peer_discovery.clone();

        let state = State::new(
            config,
            consensus_config,
            connect,
            peers,
            &last_block,
            last_block_skip.as_ref(),
            system_state.current_time(),
        );

        let validator_id = state.validator_id();
        let node_role = NodeRole::new(validator_id);
        let is_enabled = api_state.is_enabled();
        api_state.set_node_role(node_role);

        Self {
            blockchain,
            api_state,
            plugins: vec![],
            system_state,
            state,
            channel: sender,
            peer_discovery,
            is_enabled,
            node_role,
            config_manager,
            allow_expedited_propose: true,
            pool_manager,
        }
    }

    fn sign_message<T>(&self, message: T) -> Verified<T>
    where
        T: TryFrom<SignedMessage> + IntoMessage,
    {
        Verified::from_value(
            message,
            self.state.keys().consensus_pk(),
            self.state.keys().consensus_sk(),
        )
    }

    /// Return internal `SharedNodeState`
    fn api_state(&self) -> &SharedNodeState {
        &self.api_state
    }

    /// Returns value of the `first_round_timeout` field from the current `ConsensusConfig`.
    fn first_round_timeout(&self) -> Milliseconds {
        self.state().consensus_config().first_round_timeout
    }

    /// Returns value of the `round_timeout_increase` field from the current `ConsensusConfig`.
    fn round_timeout_increase(&self) -> Milliseconds {
        (self.state().consensus_config().first_round_timeout
            * ConsensusConfig::TIMEOUT_LINEAR_INCREASE_PERCENT)
            / 100
    }

    /// Returns value of the `status_timeout` field from the current `ConsensusConfig`.
    fn status_timeout(&self) -> Milliseconds {
        self.state().consensus_config().status_timeout
    }

    /// Returns value of the `peers_timeout` field from the current `ConsensusConfig`.
    fn peers_timeout(&self) -> Milliseconds {
        self.state().consensus_config().peers_timeout
    }

    /// Returns value of the minimal propose timeout.
    fn min_propose_timeout(&self) -> Milliseconds {
        self.state().consensus_config().min_propose_timeout
    }

    /// Returns value of the maximal propose timeout.
    fn max_propose_timeout(&self) -> Milliseconds {
        self.state().consensus_config().max_propose_timeout
    }

    /// Returns threshold starting from which the minimal propose timeout value is used.
    fn propose_timeout_threshold(&self) -> u32 {
        self.state().consensus_config().propose_timeout_threshold
    }

    /// Returns `State` of the node.
    pub(crate) fn state(&self) -> &State {
        &self.state
    }

    /// Returns a mutable reference to the `State` of the node.
    #[cfg(test)]
    pub(crate) fn state_mut(&mut self) -> &mut State {
        &mut self.state
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

        // Re-initialize epoch start to make it closer to the actual start time of the node.
        // While this is mainly useful for tests, there may be other cases in which the node
        // is not immediately started after construction.
        self.state
            .set_epoch_start_time(self.system_state.current_time());

        let snapshot = self.blockchain.snapshot();
        let schema = NodeSchema::new(&snapshot);
        // Recover previous saved round if any.
        let round = schema.consensus_round();
        self.state.jump_round(round);
        info!("Jump to round {}", round);

        self.add_timeouts();
        if self.state.is_leader() && round == Round(1) {
            // Propose the block after a short delay, which is used to gather initial transactions
            // (if any), connect to peers, etc.
            //
            // Note that this handles the case when the node has already proposed a block
            // at the same `(epoch, round)`; in this case, the node has at least its own `Prevote`,
            // so the `Propose` creation will be skipped in `handle_propose_timeout`.
            //
            // Restricting `round == Round(1)` is in order to prevent proposals with a large delay.
            // (Indeed, the delay in this case will be at least `first_round_timeout`, i.e.,
            // order of 3 seconds.) Heuristically, `round > Round(1)` increases probability that
            // the node's `(epoch, round)` is outdated.
            self.add_propose_timeout();
        }

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
        self.maybe_add_flush_pool_timeout();
    }

    /// Sends the given message to a peer by its public key.
    fn send_to_peer<T: Into<SignedMessage>>(&mut self, public_key: PublicKey, message: T) {
        let message = message.into();
        let request = NetworkRequest::SendMessage(public_key, message);
        self.channel.network_requests.send(request);
    }

    /// Broadcasts given message to all peers.
    fn broadcast<M: Into<SignedMessage>>(&mut self, message: M) {
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
    fn connect(&mut self, key: PublicKey) {
        let connect = self.state.our_connect_message().clone();
        self.send_to_peer(key, connect);
    }

    /// Adds a timeout request.
    fn add_timeout(&mut self, timeout: NodeTimeout, time: SystemTime) {
        let request = TimeoutRequest(time, timeout);
        self.channel.internal_requests.send(request.into());
    }

    /// Adds request timeout if it isn't already requested.
    fn request(&mut self, data: RequestData, peer: PublicKey) {
        let is_new = self.state.request(data.clone(), peer);
        if is_new {
            self.add_request_timeout(data, None);
        }
    }

    /// Adds `NodeTimeout::Round` timeout to the channel.
    fn add_round_timeout(&mut self) {
        let time = self.round_start_time(self.state.round().next());
        trace!(
            "ADD ROUND TIMEOUT: time={:?}, height={}, round={}",
            time,
            self.state.epoch(),
            self.state.round()
        );
        let timeout = NodeTimeout::Round(self.state.epoch(), self.state.round());
        self.add_timeout(timeout, time);
    }

    /// Adds `NodeTimeout::Propose` timeout to the channel.
    fn add_propose_timeout(&mut self) {
        let timeout = if self.need_faster_propose() {
            self.min_propose_timeout()
        } else {
            self.max_propose_timeout()
        };

        let time = self.round_start_time(self.state.round()) + Duration::from_millis(timeout);

        trace!(
            "ADD PROPOSE TIMEOUT: time={:?}, height={}, round={}",
            time,
            self.state.epoch(),
            self.state.round()
        );
        let timeout = NodeTimeout::Propose(self.state.epoch(), self.state.round());
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
    fn add_status_timeout(&mut self) {
        let time = self.system_state.current_time() + Duration::from_millis(self.status_timeout());
        let height = self.state.epoch();
        self.add_timeout(NodeTimeout::Status(height), time);
    }

    /// Adds `NodeTimeout::Request` timeout with `RequestData` to the channel.
    fn add_request_timeout(&mut self, data: RequestData, peer: Option<PublicKey>) {
        trace!("ADD REQUEST TIMEOUT");
        let time = self.system_state.current_time() + data.timeout();
        self.add_timeout(NodeTimeout::Request(data, peer), time);
    }

    /// Adds `NodeTimeout::PeerExchange` timeout to the channel.
    fn add_peer_exchange_timeout(&mut self) {
        trace!("ADD PEER EXCHANGE TIMEOUT");
        let time = self.system_state.current_time() + Duration::from_millis(self.peers_timeout());
        self.add_timeout(NodeTimeout::PeerExchange, time);
    }

    /// Adds `NodeTimeout::UpdateApiState` timeout to the channel.
    fn add_update_api_state_timeout(&mut self) {
        let time = self.system_state.current_time()
            + Duration::from_millis(self.api_state().state_update_timeout());
        self.add_timeout(NodeTimeout::UpdateApiState, time);
    }

    fn maybe_add_flush_pool_timeout(&mut self) {
        if let Some(timeout) = self.state().flush_pool_timeout() {
            let time = self.system_state.current_time() + timeout;
            self.add_timeout(NodeTimeout::FlushPool, time);
        }
    }

    /// Returns hash of the last block.
    fn last_block_hash(&self) -> Hash {
        self.blockchain.as_ref().last_block().object_hash()
    }

    /// Returns the number of uncommitted transactions.
    fn uncommitted_txs_count(&self) -> u64 {
        self.blockchain.as_ref().pool_size() + self.state.tx_cache_len() as u64
    }

    /// Returns start time of the requested round.
    fn round_start_time(&self, round: Round) -> SystemTime {
        // Round start time = H + (r - 1) * t0 + (r-1)(r-2)/2 * dt
        // Where:
        // H - height start time
        // t0 - Round(1) timeout length, dt - timeout increase value
        // r - round number, r = 1,2,...
        let previous_round: u64 = round.previous().into();
        let ms = previous_round * self.first_round_timeout()
            + (previous_round * previous_round.saturating_sub(1)) / 2
                * self.round_timeout_increase();
        self.state.epoch_start_time() + Duration::from_millis(ms)
    }
}

impl fmt::Debug for NodeHandler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeHandler")
            .field("channel", &self.channel)
            .field("blockchain", &self.blockchain)
            .field("peer_discovery", &self.peer_discovery)
            .finish()
    }
}

/// Handle allowing to shut down the node.
#[derive(Debug, Clone)]
pub struct ShutdownHandle {
    inner: ApiSender<ExternalMessage>,
}

impl ShutdownHandle {
    /// Shuts down the node.
    ///
    /// # Return value
    ///
    /// The failure means that the node is already being shut down.
    pub async fn shutdown(mut self) -> Result<(), SendError> {
        self.inner.send_message(ExternalMessage::Shutdown).await
    }
}

/// Default system state provider implementation which uses `SystemTime::now`
/// to get the current time.
#[derive(Debug)]
struct DefaultSystemState(SocketAddr);

impl SystemStateProvider for DefaultSystemState {
    fn listen_address(&self) -> SocketAddr {
        self.0
    }

    fn current_time(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Channel between the node and external event producers / consumers.
#[derive(Debug)]
pub struct NodeChannel {
    /// Channel for network requests.
    pub(crate) network_requests: (mpsc::Sender<NetworkRequest>, mpsc::Receiver<NetworkRequest>),

    /// Channel for timeout requests.
    #[doc(hidden)] // public because of the `transactions` benchmark
    pub internal_requests: (
        mpsc::Sender<InternalRequest>,
        mpsc::Receiver<InternalRequest>,
    ),

    /// Channel for transferring API endpoints from producers (e.g., Rust runtime) to the
    /// `ApiManager`.
    endpoints: (
        mpsc::Sender<UpdateEndpoints>,
        mpsc::Receiver<UpdateEndpoints>,
    ),

    /// Channel for externally generated transactions.
    #[doc(hidden)] // public because of the `transactions` benchmark
    pub transactions: (
        mpsc::Sender<Verified<AnyTx>>,
        mpsc::Receiver<Verified<AnyTx>>,
    ),

    /// Channel for API requests.
    #[doc(hidden)] // public because of the `transactions` benchmark
    pub api_requests: (
        mpsc::Sender<ExternalMessage>,
        mpsc::Receiver<ExternalMessage>,
    ),

    /// Channel for network events.
    #[doc(hidden)] // public because of the `transactions` benchmark
    pub network_events: (mpsc::Sender<NetworkEvent>, mpsc::Receiver<NetworkEvent>),

    /// Channel for internal events.
    #[doc(hidden)] // public because of the `transactions` benchmark
    pub internal_events: (mpsc::Sender<InternalEvent>, mpsc::Receiver<InternalEvent>),
}

/// Interface of the configuration manager usable for updating node configuration on
/// the fly.
pub trait ConfigManager: Send {
    /// Update connect list in the node configuration.
    fn store_connect_list(&mut self, connect_list: ConnectListConfig);
}

/// Node capable of processing requests from external clients and participating in the consensus
/// algorithm.
///
/// # Signal Handling
///
/// By default, the node installs several signal hooks. This can be disabled by using
/// [`disable_signals()`] method in `NodeBuilder`.
///
/// On Unix, the node will gracefully shut down on receiving any of SIGINT, SIGTERM
/// or SIGQUIT and will ignore SIGHUP. On Windows, the node will gracefully shut down on
/// receiving `Ctrl + C` break. Graceful shutdown means, among other things, flushing uncommitted
/// transactions into the persistent storage.
///
/// [`disable_signals()`]: struct.NodeBuilder.html#method.disable_signals
#[derive(Debug)]
pub struct Node {
    api_manager_config: ApiManagerConfig,
    api_options: NodeApiConfig,
    network_config: NetworkConfiguration,
    handler: NodeHandler,
    channel: NodeChannel,
    max_message_len: u32,
    thread_pool_size: Option<u8>,
    disable_signals: bool,
}

impl Default for NodeChannel {
    fn default() -> Self {
        Self::new(&EventsPoolCapacity::default())
    }
}

impl NodeChannel {
    /// Creates `NodeChannel` with the given pool capacities.
    pub fn new(buffer_sizes: &EventsPoolCapacity) -> Self {
        Self {
            network_requests: mpsc::channel(buffer_sizes.network_requests_capacity),
            internal_requests: mpsc::channel(buffer_sizes.internal_events_capacity),
            endpoints: mpsc::channel(buffer_sizes.internal_events_capacity),
            transactions: mpsc::channel(buffer_sizes.api_requests_capacity),
            api_requests: mpsc::channel(buffer_sizes.api_requests_capacity),
            network_events: mpsc::channel(buffer_sizes.network_events_capacity),
            internal_events: mpsc::channel(buffer_sizes.internal_events_capacity),
        }
    }

    /// Returns the sender for API requests.
    pub fn api_sender(&self) -> ApiSender {
        ApiSender::new(self.transactions.0.clone())
    }

    /// Returns the sender for HTTP endpoints.
    pub fn endpoints_sender(&self) -> mpsc::Sender<UpdateEndpoints> {
        self.endpoints.0.clone()
    }

    /// Returns the channel for sending timeouts, networks and API requests.
    fn node_sender(&self) -> NodeSender {
        NodeSender {
            internal_requests: SyncSender::new(
                self.internal_requests.0.clone(),
                "internal request",
            ),
            network_requests: SyncSender::new(self.network_requests.0.clone(), "network request"),
            _transactions: self.transactions.0.clone(),
            _api_requests: self.api_requests.0.clone(),
        }
    }
}

/// Builder for `Node`.
pub struct NodeBuilder {
    channel: NodeChannel,
    blockchain_builder: BlockchainBuilder,
    node_config: NodeConfig,
    node_keys: Keys,
    config_manager: Option<Box<dyn ConfigManager>>,
    pool_manager: Box<dyn ManagePool>,
    plugins: Vec<Box<dyn NodePlugin>>,
    disable_signals: bool,
}

impl fmt::Debug for NodeBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeBuilder")
            .field("channel", &self.channel)
            .field("blockchain_builder", &self.blockchain_builder)
            .field("node_config", &self.node_config)
            .finish()
    }
}

impl NodeBuilder {
    /// Instantiates a builder.
    pub fn new(
        database: impl Into<Arc<dyn Database>>,
        node_config: NodeConfig,
        node_keys: Keys,
    ) -> Self {
        node_config
            .validate()
            .expect("Node configuration is inconsistent");

        let channel = NodeChannel::new(&node_config.mempool.events_pool_capacity);
        let blockchain = Blockchain::new(database, node_keys.service.clone(), channel.api_sender());
        let blockchain_builder = BlockchainBuilder::new(blockchain);

        Self {
            channel,
            blockchain_builder,
            node_config,
            node_keys,
            config_manager: None,
            plugins: vec![],
            pool_manager: Box::new(StandardPoolManager::default()),
            disable_signals: false,
        }
    }

    /// Adds a genesis config to use if the blockchain is not initialized yet.
    pub fn with_genesis_config(mut self, genesis_config: GenesisConfig) -> Self {
        self.blockchain_builder = self.blockchain_builder.with_genesis_config(genesis_config);
        self
    }

    /// Adds a runtime to the blockchain.
    pub fn with_runtime<T>(mut self, runtime: T) -> Self
    where
        T: Into<RuntimeInstance>,
    {
        self.blockchain_builder = self.blockchain_builder.with_runtime(runtime);
        self
    }

    /// Adds a runtime which depends on a `NodeChannel` (e.g., to update HTTP API of the node).
    pub fn with_runtime_fn<T, F>(mut self, runtime_fn: F) -> Self
    where
        T: Into<RuntimeInstance>,
        F: FnOnce(&NodeChannel) -> T,
    {
        let runtime = runtime_fn(&self.channel);
        self.blockchain_builder = self.blockchain_builder.with_runtime(runtime);
        self
    }

    /// Adds the configuration manager.
    pub fn with_config_manager<T: ConfigManager + 'static>(mut self, manager: T) -> Self {
        self.config_manager = Some(Box::new(manager));
        self
    }

    /// Sets custom pool management logic for the node. Using this method, it is possible
    /// to customize block proposals and removing transactions from the pool of unconfirmed
    /// transactions.
    ///
    /// # Stability and safety
    ///
    /// This method is unstable. Using a custom pool manager **CAN LEAD TO CONSENSUS FAILURE.**
    /// See the [`pool`] module docs for more details.
    ///
    /// [`pool`]: pool/index.html
    pub fn with_pool_manager<T: ManagePool + 'static>(mut self, manager: T) -> Self {
        self.pool_manager = Box::new(manager);
        self
    }

    /// Adds a plugin.
    pub fn with_plugin<T: NodePlugin + 'static>(mut self, plugin: T) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Switches off [default signal handling] for the node.
    /// This is useful to implement more complex signal handling, or one that differs
    /// from the default.
    ///
    /// If you use custom signal handlers, the node may be shut down gracefully using
    /// [`ShutdownHandle`].
    ///
    /// [`ShutdownHandle`]: struct.ShutdownHandle.html
    /// [default signal handling]: struct.Node.html#signal-handling
    pub fn disable_signals(mut self) -> Self {
        self.disable_signals = true;
        self
    }

    /// Converts this builder into a `Node`.
    pub fn build(self) -> Node {
        let blockchain = self.blockchain_builder.build();
        let mut node = Node::with_blockchain(
            blockchain,
            self.channel,
            self.node_config,
            self.node_keys,
            self.config_manager,
            self.plugins,
            self.pool_manager,
        );
        node.disable_signals = self.disable_signals;
        node
    }
}

impl Node {
    /// Creates a node for the given blockchain and node configuration.
    fn with_blockchain(
        blockchain: BlockchainMut,
        channel: NodeChannel,
        node_cfg: NodeConfig,
        node_keys: Keys,
        config_manager: Option<Box<dyn ConfigManager>>,
        plugins: Vec<Box<dyn NodePlugin>>,
        pool_manager: Box<dyn ManagePool>,
    ) -> Self {
        crypto::init();

        let peers = node_cfg.connect_list.addresses();
        let config = Configuration {
            connect_list: ConnectList::from_config(node_cfg.connect_list),
            mempool: node_cfg.mempool,
            network: node_cfg.network,
            peer_discovery: peers,
            keys: node_keys,
        };

        let api_state = SharedNodeState::new(node_cfg.api.state_update_timeout as u64);
        let mut api_aggregator = ApiAggregator::new();
        let plugin_api_context = PluginApiContext::new(
            blockchain.as_ref(),
            &api_state,
            ApiSender::new(channel.api_requests.0.clone()),
        );
        for plugin in &plugins {
            let endpoints = plugin.wire_api(plugin_api_context.clone());
            api_aggregator.extend(endpoints);
        }

        let system_state = Box::new(DefaultSystemState(node_cfg.listen_address));
        let network_config = config.network;
        let api_cfg = node_cfg.api.clone();

        let mut servers = HashMap::new();
        if let Some(listen_address) = api_cfg.public_api_address {
            let mut server_config = WebServerConfig::new(listen_address);
            server_config.allow_origin = api_cfg.public_allow_origin.clone();
            servers.insert(ApiAccess::Public, server_config);
        }
        if let Some(listen_address) = api_cfg.private_api_address {
            let mut server_config = WebServerConfig::new(listen_address);
            server_config.allow_origin = api_cfg.private_allow_origin.clone();
            servers.insert(ApiAccess::Private, server_config);
        }

        let restart_policy = node_cfg.api.server_restart;
        let api_runtime_config = ApiManagerConfig::new(servers, api_aggregator)
            .with_retries(restart_policy.retry_timeout, restart_policy.max_retries);

        let mut handler = NodeHandler::new(
            blockchain,
            &node_cfg.external_address,
            channel.node_sender(),
            system_state,
            config,
            api_state,
            config_manager,
            pool_manager,
        );
        handler.plugins = plugins;

        Self {
            api_options: api_cfg,
            handler,
            channel,
            network_config,
            max_message_len: node_cfg.consensus.max_message_len,
            thread_pool_size: node_cfg.thread_pool_size,
            api_manager_config: api_runtime_config,
            disable_signals: false,
        }
    }

    /// Launches only consensus messages handler.
    /// This may be used if you want to customize api with the `ApiContext`.
    async fn run_handler(mut self, handshake_params: HandshakeParams) -> anyhow::Result<()> {
        // Stop timeout sufficient to prevent undefined behavior in RocksDB destructor code
        // (see below).
        const STOP_TIMEOUT: Duration = Duration::from_millis(50);

        self.handler.initialize();
        let res = Reactor::new(self).run(handshake_params).await;

        // Wait for a little bit to prevent undefined behavior with RocksDB, when it is dropped
        // concurrently with the process exiting. By delaying, we give time for
        // the `actix` servers to exit (these servers contain the bulk of `Arc` pointers to RocksDB).
        // The exit happens once the node infrastructure (in particular, the sender of endpoints to
        // `exonum_api::ApiManager`) is dropped, i.e., before this point.
        //
        // We assume that waiting `STOP_TIMEOUT` is acceptable for typical use cases;
        // even if the process does not terminate with the node exit,
        // running a node is generally understood as a long-term task.
        delay_for(STOP_TIMEOUT).await;
        res
    }

    /// Launches a `Node` and optionally creates threads for public and private API handlers,
    /// depending on the provided `NodeConfig`.
    pub async fn run(self) -> anyhow::Result<()> {
        trace!("Running node.");

        // Runs NodeHandler.
        let handshake_params = HandshakeParams::new(
            &self.state().keys().consensus,
            self.state().connect_list(),
            self.state().our_connect_message().clone(),
            self.max_message_len,
        );
        self.run_handler(handshake_params).await
    }

    /// Returns `State` of the node.
    fn state(&self) -> &State {
        self.handler.state()
    }

    /// Returns the blockchain handle, which can be used to read blockchain state and send
    /// transactions to the node.
    pub fn blockchain(&self) -> &Blockchain {
        self.handler.blockchain.as_ref()
    }

    /// Returns a shutdown handle for the node. It is possible to instantiate multiple handles
    /// using this method; only the first call to shutdown the node is guaranteed to succeed
    /// (but this single call is enough to stop the node).
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle {
            inner: ApiSender::new(self.channel.api_requests.0.clone()),
        }
    }
}

struct Reactor {
    handler_part: HandlerPart<NodeHandler>,
    network_part: NetworkPart,
    internal_part: InternalPart,
    api_part: oneshot::Receiver<io::Result<()>>,
    shutdown_handle: ShutdownHandle,
    // Flag indicating whether the reactor should explicitly handle signals.
    // If there is at least one actix HTTP server, signal handling will be performed by it,
    // so there is no need to perform it explicitly.
    needs_signal_handler: bool,
}

impl Reactor {
    fn new(node: Node) -> Self {
        let connect_message = node.state().our_connect_message().clone();
        let connect_list = node.state().connect_list();
        let shutdown_handle = node.shutdown_handle();

        let mut api_config = node.api_manager_config;
        api_config.disable_signals = node.disable_signals;
        let needs_signal_handler = !node.disable_signals && api_config.servers.is_empty();
        let api_manager = ApiManager::new(api_config);
        let endpoints = node.channel.endpoints.1;
        let api_task = api_manager.run(endpoints);
        let (api_part_tx, api_part) = oneshot::channel();

        // Creating a separate thread here seems easier than making `Node::run()` return
        // a non-`Send` future (which, e.g., precludes running nodes with `tokio::spawn`).
        thread::spawn(|| {
            let res = System::new("exonum-node").block_on(api_task);
            if let Err(ref err) = res {
                log::error!("Error in actix thread: {}", err);
            }
            api_part_tx.send(res).ok();
        });

        let (network_tx, network_rx) = node.channel.network_events;
        let internal_requests_rx = node.channel.internal_requests.1;
        let network_part = NetworkPart {
            our_connect_message: connect_message,
            listen_address: node.handler.system_state.listen_address(),
            network_requests: node.channel.network_requests.1,
            network_tx,
            network_config: node.network_config,
            max_message_len: node.max_message_len,
            connect_list,
        };

        let (internal_tx, internal_rx) = node.channel.internal_events;
        let handler_part = HandlerPart {
            handler: node.handler,
            internal_rx,
            network_rx,
            transactions_rx: node.channel.transactions.1,
            api_rx: node.channel.api_requests.1,
        };

        let internal_part = InternalPart {
            internal_tx,
            internal_requests_rx,
        };

        Self {
            handler_part,
            network_part,
            internal_part,
            api_part,
            shutdown_handle,
            needs_signal_handler,
        }
    }

    /// Listens to the same 3 signals as `actix`: SIGINT, SIGTERM, and SIGQUIT.
    #[cfg(unix)]
    #[allow(clippy::mut_mut)] // occurs in the `select!` macro
    async fn listen_to_signals() {
        use futures::StreamExt;
        use tokio::signal::unix::{signal, SignalKind};

        let int_listener = tokio::signal::ctrl_c().fuse();
        futures::pin_mut!(int_listener);

        // If setting the signal listener fails, we replace the listener with a stream
        // that never resolves.
        let mut term_listener = signal(SignalKind::terminate())
            .map_or_else(|_| stream::pending().right_stream(), StreamExt::left_stream)
            .fuse();

        let mut quit_listener = signal(SignalKind::quit())
            .map_or_else(|_| stream::pending().right_stream(), StreamExt::left_stream)
            .fuse();

        // We also register a SIGHUP listener which ignores the signal.
        if let Ok(mut hangup_listener) = signal(SignalKind::hangup()) {
            tokio::spawn(async move {
                while let Some(()) = hangup_listener.next().await {
                    log::info!("Received SIGHUP; ignoring");
                }
            });
        }

        futures::select! {
            _ = int_listener => (),
            _ = term_listener.next() => (),
            _ = quit_listener.next() => (),
        }
    }

    #[cfg(not(unix))]
    async fn listen_to_signals() {
        tokio::signal::ctrl_c().await.ok();
    }

    #[allow(clippy::mut_mut)] // occurs in the `select!` macro
    async fn run(self, handshake_params: HandshakeParams) -> anyhow::Result<()> {
        let internal_task = self.internal_part.run().fuse();
        futures::pin_mut!(internal_task);
        let network_task = self.network_part.run(handshake_params).fuse();
        futures::pin_mut!(network_task);
        let handler_task = self.handler_part.run().fuse();
        futures::pin_mut!(handler_task);
        let mut api_task = self.api_part.fuse();

        if self.needs_signal_handler {
            // Send the shutdown signal once we received a signal.
            let shutdown_handle = self.shutdown_handle.clone();
            let signal_rx = Self::listen_to_signals();

            tokio::spawn(async {
                signal_rx.await;
                shutdown_handle.shutdown().await.ok();
            });
        };

        // The remaining tasks are dropped after the first task is terminated,
        // which should free all associated resources.
        let (res, should_clean_up) = futures::select! {
            () = internal_task => (Ok(()), true),
            () = network_task => (Ok(()), true),
            () = handler_task => (Ok(()), false),

            res = api_task => {
                let res = match res {
                    Err(_) => Err(format_err!("Actix thread panicked")),
                    Ok(res) => res.map_err(From::from),
                };
                (res, true)
            }
        };

        if should_clean_up {
            // Ensure that we run the shutdown code, such as flushing messages to the storage.
            self.shutdown_handle.shutdown().await.ok();
            handler_task.await;
        }

        log::info!("Node terminated with status {:?}", res);
        res
    }
}

#[doc(hidden)]
pub fn generate_testnet_config(count: u16, start_port: u16) -> Vec<(NodeConfig, Keys)> {
    use exonum::blockchain::ValidatorKeys;

    let keys: Vec<_> = (0..count as usize).map(|_| Keys::random()).collect();
    let validator_keys = keys
        .iter()
        .map(|keys| ValidatorKeys::new(keys.consensus_pk(), keys.service_pk()))
        .collect();
    let consensus = ConsensusConfig::default().with_validator_keys(validator_keys);

    let peers = (0..keys.len())
        .map(|x| format!("127.0.0.1:{}", start_port + x as u16))
        .collect::<Vec<_>>();

    keys.into_iter()
        .enumerate()
        .map(|(idx, keys)| {
            let config = NodeConfig {
                listen_address: peers[idx].parse().unwrap(),
                external_address: peers[idx].clone(),
                network: NetworkConfiguration::default(),
                consensus: consensus.clone(),
                connect_list: ConnectListConfig::from_validator_keys(
                    &consensus.validator_keys,
                    &peers,
                ),
                api: NodeApiConfig::default(),
                mempool: MemoryPoolConfig::default(),
                thread_pool_size: None,
            };
            (config, keys)
        })
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use exonum::merkledb::TemporaryDB;

    use super::*;

    #[test]
    fn test_good_internal_events_config() {
        let db = TemporaryDB::new();
        let (node_cfg, node_keys) = generate_testnet_config(1, 16_500).pop().unwrap();
        NodeBuilder::new(db, node_cfg, node_keys);
    }

    #[test]
    #[should_panic(expected = "internal_events_capacity(0) must be strictly larger than 2")]
    fn test_bad_internal_events_capacity_too_small() {
        let db = TemporaryDB::new();
        let (mut node_cfg, node_keys) = generate_testnet_config(1, 16_500).pop().unwrap();
        node_cfg
            .mempool
            .events_pool_capacity
            .internal_events_capacity = 0;
        NodeBuilder::new(db, node_cfg, node_keys);
    }

    #[test]
    #[should_panic(expected = "network_requests_capacity(0) must be strictly larger than 0")]
    fn test_bad_network_requests_capacity_too_small() {
        let db = TemporaryDB::new();
        let (mut node_cfg, node_keys) = generate_testnet_config(1, 16_500)[0].clone();
        node_cfg
            .mempool
            .events_pool_capacity
            .network_requests_capacity = 0;
        NodeBuilder::new(db, node_cfg, node_keys);
    }

    #[test]
    #[should_panic(expected = "must be smaller than 65536")]
    fn test_bad_internal_events_capacity_too_large() {
        let accidental_large_value = usize::max_value();
        let db = TemporaryDB::new();

        let (mut node_cfg, node_keys) = generate_testnet_config(1, 16_500).pop().unwrap();
        node_cfg
            .mempool
            .events_pool_capacity
            .internal_events_capacity = accidental_large_value;
        NodeBuilder::new(db, node_cfg, node_keys);
    }

    #[test]
    #[should_panic(expected = "must be smaller than 65536")]
    fn test_bad_network_requests_capacity_too_large() {
        let accidental_large_value = usize::max_value();
        let db = TemporaryDB::new();

        let (mut node_cfg, node_keys) = generate_testnet_config(1, 16_500)[0].clone();
        node_cfg
            .mempool
            .events_pool_capacity
            .network_requests_capacity = accidental_large_value;
        NodeBuilder::new(db, node_cfg, node_keys);
    }

    #[test]
    fn flush_pool_strategy_is_serializable() {
        let mut mempool_config = MemoryPoolConfig::default();
        let s = toml::to_string(&mempool_config).unwrap();
        let restored: MemoryPoolConfig = toml::from_str(&s).unwrap();
        assert_eq!(restored, mempool_config);

        mempool_config.flush_pool_strategy = FlushPoolStrategy::Never;
        let s = toml::to_string(&mempool_config).unwrap();
        let restored: MemoryPoolConfig = toml::from_str(&s).unwrap();
        assert_eq!(restored, mempool_config);

        let config_without_strategy = r#"
            [events_pool_capacity]
            network_requests_capacity = 512
            network_events_capacity = 512
            internal_events_capacity = 128
            api_requests_capacity = 1024
        "#;
        let restored: MemoryPoolConfig = toml::from_str(config_without_strategy).unwrap();
        assert_eq!(restored, MemoryPoolConfig::default());
    }
}
