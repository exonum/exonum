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

//! Exonum node API implementation.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use crate::{
    blockchain::ValidatorKeys,
    events::network::ConnectedPeerAddr,
    helpers::Milliseconds,
    node::{ConnectInfo, NodeRole, State},
};

pub mod private;
pub mod public;

#[derive(Default)]
struct ApiNodeState {
    // TODO: Update on event? (ECR-1632)
    incoming_connections: HashSet<ConnectInfo>,
    outgoing_connections: HashSet<ConnectInfo>,
    reconnects_timeout: HashMap<SocketAddr, Milliseconds>,
    is_enabled: bool,
    node_role: NodeRole,
    majority_count: usize,
    validators: Vec<ValidatorKeys>,
    tx_cache_len: usize,
}

impl fmt::Debug for ApiNodeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiNodeState")
            .field("incoming_connections", &self.incoming_connections)
            .field("outgoing_connections", &self.outgoing_connections)
            .field("reconnects_timeout", &self.reconnects_timeout)
            .field("is_enabled", &self.is_enabled)
            .field("node_role", &self.node_role)
            .field("majority_count", &self.majority_count)
            .field("validators", &self.validators)
            .field("tx_cache_len", &self.tx_cache_len)
            .finish()
    }
}

impl ApiNodeState {
    fn new() -> Self {
        Self {
            is_enabled: true,
            ..Default::default()
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
    /// Timeout to update API state.
    pub state_update_timeout: Milliseconds,
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

    /// Returns a list of other nodes to which the connection has failed
    /// and a reconnect attempt is required. The method also indicates the time
    /// after which a new connection attempt is performed.
    pub fn reconnects_timeout(&self) -> Vec<(SocketAddr, Milliseconds)> {
        self.node
            .read()
            .expect("Expected read lock.")
            .reconnects_timeout
            .iter()
            .map(|(c, e)| (*c, *e))
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

        for (p, a) in state.connections() {
            match a {
                ConnectedPeerAddr::In(addr) => {
                    let conn_info = ConnectInfo {
                        address: addr.to_string(),
                        public_key: *p,
                    };
                    lock.incoming_connections.insert(conn_info);
                }
                ConnectedPeerAddr::Out(_, addr) => {
                    let conn_info = ConnectInfo {
                        address: addr.to_string(),
                        public_key: *p,
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

    /// Adds a reconnect timeout.
    pub fn add_reconnect_timeout(
        &self,
        addr: SocketAddr,
        timeout: Milliseconds,
    ) -> Option<Milliseconds> {
        self.node
            .write()
            .expect("Expected write lock")
            .reconnects_timeout
            .insert(addr, timeout)
    }

    /// Removes the reconnect timeout and returns the previous value.
    pub fn remove_reconnect_timeout(&self, addr: &SocketAddr) -> Option<Milliseconds> {
        self.node
            .write()
            .expect("Expected write lock")
            .reconnects_timeout
            .remove(addr)
    }

    pub(crate) fn tx_cache_size(&self) -> usize {
        let state = self.node.read().expect("Expected read lock");
        state.tx_cache_len
    }
}
