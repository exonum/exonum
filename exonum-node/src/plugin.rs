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

use exonum::{
    blockchain::{ApiSender, Blockchain, ValidatorKeys},
    helpers::Milliseconds,
    merkledb::Snapshot,
};
use exonum_api::ApiBuilder;

use std::{
    collections::HashSet,
    fmt,
    sync::{Arc, RwLock},
};

use crate::{
    events::network::ConnectedPeerAddr, state::State, ConnectInfo, ExternalMessage, NodeRole,
};

#[derive(Debug, Default)]
struct ApiNodeState {
    // TODO: Update on event? (ECR-1632)
    incoming_connections: HashSet<ConnectInfo>,
    outgoing_connections: HashSet<ConnectInfo>,
    is_enabled: bool,
    node_role: NodeRole,
    majority_count: usize,
    validators: Vec<ValidatorKeys>,
    tx_cache_len: usize,
}

impl ApiNodeState {
    fn new() -> Self {
        Self {
            is_enabled: true,
            ..Self::default()
        }
    }
}

/// Shared part of the context, used to take some values from the `Node`.
/// As there is no way to directly access the node state, this entity is
/// regularly updated with information about the node and transfers this
/// information to API.
#[derive(Clone, Debug)]
pub struct SharedNodeState {
    node: Arc<RwLock<ApiNodeState>>,
    state_update_timeout: Milliseconds,
}

impl SharedNodeState {
    /// Creates a new `SharedNodeState` instance.
    pub fn new(state_update_timeout: Milliseconds) -> Self {
        Self {
            node: Arc::new(RwLock::new(ApiNodeState::new())),
            state_update_timeout,
        }
    }

    /// Returns a list of connected addresses of other nodes.
    pub fn incoming_connections(&self) -> Vec<ConnectInfo> {
        self.node
            .read()
            .expect("Expected read lock.")
            .incoming_connections
            .iter()
            .cloned()
            .collect()
    }

    /// Returns a list of our connection sockets.
    pub fn outgoing_connections(&self) -> Vec<ConnectInfo> {
        self.node
            .read()
            .expect("Expected read lock.")
            .outgoing_connections
            .iter()
            .cloned()
            .collect()
    }

    /// Returns a boolean value which indicates whether the consensus is achieved.
    pub fn consensus_status(&self) -> bool {
        let lock = self.node.read().expect("Expected read lock.");
        let mut active_validators = lock
            .incoming_connections
            .iter()
            .chain(lock.outgoing_connections.iter())
            .filter(|ci| {
                lock.validators
                    .iter()
                    .any(|v| v.consensus_key == ci.public_key)
            })
            .count();

        if lock.node_role.is_validator() {
            // Peers list doesn't include current node address, so we have to increment its length.
            // E.g. if we have 3 items in peers list, it means that we have 4 nodes overall.
            active_validators += 1;
        }

        // Just after Node is started (node status isn't updated) majority_count = 0,
        // so we have to check that majority count is greater than 0.
        active_validators >= lock.majority_count && lock.majority_count > 0
    }

    /// Returns a boolean value which indicates whether the node is enabled
    /// or not.
    pub fn is_enabled(&self) -> bool {
        let state = self.node.read().expect("Expected read lock.");
        state.is_enabled
    }

    /// Updates internal state, from `State` of a blockchain node.
    pub(crate) fn update_node_state(&self, state: &State) {
        let mut lock = self.node.write().expect("Expected write lock.");

        lock.incoming_connections.clear();
        lock.outgoing_connections.clear();
        lock.majority_count = state.majority_count();
        lock.node_role = NodeRole::new(state.validator_id());
        lock.validators = state.validators().to_vec();
        lock.tx_cache_len = state.tx_cache_len();

        for (public_key, addr) in state.connections() {
            match addr {
                ConnectedPeerAddr::In(addr) => {
                    let conn_info = ConnectInfo {
                        address: addr.to_string(),
                        public_key: *public_key,
                    };
                    lock.incoming_connections.insert(conn_info);
                }
                ConnectedPeerAddr::Out(_, addr) => {
                    let conn_info = ConnectInfo {
                        address: addr.to_string(),
                        public_key: *public_key,
                    };
                    lock.outgoing_connections.insert(conn_info);
                }
            }
        }
    }

    /// Transfers information to the node that the consensus process on the node
    /// should halt.
    pub(crate) fn set_enabled(&self, is_enabled: bool) {
        let mut node = self.node.write().expect("Expected write lock.");
        node.is_enabled = is_enabled;
    }

    pub(crate) fn set_node_role(&self, role: NodeRole) {
        let mut node = self.node.write().expect("Expected write lock.");
        node.node_role = role;
    }

    /// Returns the value of the `state_update_timeout`.
    pub fn state_update_timeout(&self) -> Milliseconds {
        self.state_update_timeout
    }

    /// Returns the current size of transaction cache.
    pub fn tx_cache_size(&self) -> usize {
        let state = self.node.read().expect("Expected read lock");
        state.tx_cache_len
    }
}

/// Context supplied to a node plugin in `wire_api` method.
#[derive(Debug, Clone)]
pub struct PluginApiContext<'a> {
    blockchain: &'a Blockchain,
    node_state: &'a SharedNodeState,
    api_sender: ApiSender<ExternalMessage>,
}

impl<'a> PluginApiContext<'a> {
    #[doc(hidden)] // public because of the testkit
    pub fn new(
        blockchain: &'a Blockchain,
        node_state: &'a SharedNodeState,
        api_sender: ApiSender<ExternalMessage>,
    ) -> Self {
        Self {
            blockchain,
            node_state,
            api_sender,
        }
    }

    /// Returns a reference to blockchain.
    pub fn blockchain(&self) -> &Blockchain {
        self.blockchain
    }

    /// Returns a reference to the node state.
    pub fn node_state(&self) -> &SharedNodeState {
        self.node_state
    }

    /// Returns sender of control messages to the node.
    pub fn api_sender(&self) -> ApiSender<ExternalMessage> {
        self.api_sender.clone()
    }
}

/// Plugin for Exonum node.
pub trait NodePlugin: Send {
    /// Notifies the plugin that the node has committed a block.
    ///
    /// The default implementation does nothing.
    fn after_commit(&self, _snapshot: &dyn Snapshot) {
        // Do nothing
    }

    /// Allows the plugin to extend HTTP API of the node.
    ///
    /// The default implementation returns an empty `Vec`.
    fn wire_api(&self, _context: PluginApiContext<'_>) -> Vec<(String, ApiBuilder)> {
        Vec::new()
    }
}

impl fmt::Debug for dyn NodePlugin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("NodePlugin").finish()
    }
}
