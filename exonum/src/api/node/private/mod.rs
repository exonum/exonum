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

//! Private part of the Exonum REST API.
//!
//! Private API includes requests that are available only to the blockchain
//! administrators, e.g. view the list of services on the current node.

use std::{collections::HashMap, net::SocketAddr};

use api::{Error as ApiError, ServiceApiScope, ServiceApiState};
use blockchain::{Service, SharedNodeState};
use crypto::PublicKey;
use messages::PROTOCOL_MAJOR_VERSION;
use node::{ConnectInfo, ExternalMessage};

/// Short information about the service.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ServiceInfo {
    /// Service name.
    pub name: String,
    /// Service identifier for database schema and service messages.
    pub id: u16,
}

/// Short information about the current node.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NodeInfo {
    /// Version of the `exonum` crate.
    pub core_version: Option<String>,
    /// Version of the Exonum protocol.
    pub protocol_version: u8,
    /// List of services.
    pub services: Vec<ServiceInfo>,
}

impl NodeInfo {
    /// Creates new `NodeInfo` from services list.
    pub fn new<'a, I>(services: I) -> Self
    where
        I: IntoIterator<Item = &'a Box<dyn Service>>,
    {
        let core_version = option_env!("CARGO_PKG_VERSION").map(|ver| ver.to_owned());
        Self {
            core_version,
            protocol_version: PROTOCOL_MAJOR_VERSION,
            services: services
                .into_iter()
                .map(|s| ServiceInfo {
                    name: s.service_name().to_owned(),
                    id: s.service_id(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct ReconnectInfo {
    delay: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum IncomingConnectionState {
    Active,
    Reconnect(ReconnectInfo),
}

impl Default for IncomingConnectionState {
    fn default() -> Self {
        IncomingConnectionState::Active
    }
}

#[derive(Serialize, Deserialize, Default)]
struct IncomingConnection {
    public_key: Option<PublicKey>,
    state: IncomingConnectionState,
}

#[derive(Serialize, Deserialize)]
struct PeersInfo {
    incoming_connections: Vec<SocketAddr>,
    outgoing_connections: HashMap<SocketAddr, IncomingConnection>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ConsensusEnabledQuery {
    enabled: bool,
}

/// Private system API.
#[derive(Clone, Debug)]
pub struct SystemApi {
    info: NodeInfo,
    shared_api_state: SharedNodeState,
}

impl SystemApi {
    /// Creates a new `private::SystemApi` instance.
    pub fn new(info: NodeInfo, shared_api_state: SharedNodeState) -> Self {
        Self {
            info,
            shared_api_state,
        }
    }

    /// Adds private system API endpoints to the corresponding scope.
    pub fn wire(self, api_scope: &mut ServiceApiScope) -> &mut ServiceApiScope {
        self.handle_peers_info("v1/peers", api_scope)
            .handle_peer_add("v1/peers", api_scope)
            .handle_network_info("v1/network", api_scope)
            .handle_is_consensus_enabled("v1/consensus_enabled", api_scope)
            .handle_set_consensus_enabled("v1/consensus_enabled", api_scope)
            .handle_shutdown("v1/shutdown", api_scope);
        api_scope
    }

    fn handle_peers_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_state: &ServiceApiState, _query: ()| {
            let mut outgoing_connections: HashMap<SocketAddr, IncomingConnection> = HashMap::new();

            for socket in self.shared_api_state.outgoing_connections() {
                outgoing_connections.insert(socket, Default::default());
            }

            for (s, delay) in self.shared_api_state.reconnects_timeout() {
                outgoing_connections
                    .entry(s)
                    .or_insert_with(Default::default)
                    .state = IncomingConnectionState::Reconnect(ReconnectInfo { delay });
            }

            for (s, p) in self.shared_api_state.peers_info() {
                outgoing_connections
                    .entry(s)
                    .or_insert_with(Default::default)
                    .public_key = Some(p);
            }

            Ok(PeersInfo {
                incoming_connections: self.shared_api_state.incoming_connections(),
                outgoing_connections,
            })
        });
        self_
    }

    fn handle_peer_add(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        api_scope.endpoint_mut(
            name,
            move |state: &ServiceApiState, connect_info: ConnectInfo| {
                state
                    .sender()
                    .peer_add(connect_info)
                    .map_err(ApiError::from)
            },
        );
        self
    }

    fn handle_network_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_state: &ServiceApiState, _query: ()| {
            Ok(self.info.clone())
        });
        self_
    }

    fn handle_is_consensus_enabled(
        self,
        name: &'static str,
        api_scope: &mut ServiceApiScope,
    ) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_state: &ServiceApiState, _query: ()| {
            Ok(self.shared_api_state.is_enabled())
        });
        self_
    }

    fn handle_set_consensus_enabled(
        self,
        name: &'static str,
        api_scope: &mut ServiceApiScope,
    ) -> Self {
        let self_ = self.clone();
        api_scope.endpoint_mut(
            name,
            move |state: &ServiceApiState, query: ConsensusEnabledQuery| {
                state
                    .sender()
                    .send_external_message(ExternalMessage::Enable(query.enabled))
                    .map_err(ApiError::from)
            },
        );
        self_
    }

    fn handle_shutdown(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        api_scope.endpoint_mut(name, move |state: &ServiceApiState, _query: ()| {
            state
                .sender()
                .send_external_message(ExternalMessage::Shutdown)
                .map_err(ApiError::from)
        });
        self
    }
}
