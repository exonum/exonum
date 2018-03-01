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

use serde_json;
use router::Router;
use iron::prelude::*;

use std::net::SocketAddr;
use std::collections::HashMap;

use crypto::PublicKey;
use node::ExternalMessageSender;
use blockchain::{Service, Blockchain, SharedNodeState};
use api::{Api, ApiError};
use messages::{TEST_NETWORK_ID, PROTOCOL_MAJOR_VERSION};

#[derive(Serialize, Clone, Debug)]
struct ServiceInfo {
    name: String,
    id: u16,
}

/// `DTO` is used to transfer information about node.
#[derive(Serialize, Clone, Debug)]
pub struct NodeInfo {
    network_id: u8,
    protocol_version: u8,
    services: Vec<ServiceInfo>,
}

impl NodeInfo {
    /// Creates new `NodeInfo`, from services list.
    pub fn new<'a, I>(services: I) -> NodeInfo
    where
        I: IntoIterator<Item = &'a Box<Service>>,
    {
        NodeInfo {
            network_id: TEST_NETWORK_ID,
            protocol_version: PROTOCOL_MAJOR_VERSION,
            services: services
                .into_iter()
                .map(|s| {
                    ServiceInfo {
                        name: s.service_name().to_owned(),
                        id: s.service_id(),
                    }
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Default)]
struct ReconnectInfo {
    delay: u64,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum IncomingConnectionState {
    Active,
    Reconnect(ReconnectInfo),
}

impl Default for IncomingConnectionState {
    fn default() -> IncomingConnectionState {
        IncomingConnectionState::Active
    }
}

#[derive(Serialize, Default)]
struct IncomingConnection {
    public_key: Option<PublicKey>,
    state: IncomingConnectionState,
}

#[derive(Serialize)]
struct PeersInfo {
    incoming_connections: Vec<SocketAddr>,
    outgoing_connections: HashMap<SocketAddr, IncomingConnection>,
}

/// Private system API.
#[derive(Clone, Debug)]
pub struct SystemApi {
    blockchain: Blockchain,
    info: NodeInfo,
    shared_api_state: SharedNodeState,
    node_channel: ExternalMessageSender,
}

impl SystemApi {
    /// Creates a new `public::SystemApi` instance.
    pub fn new(
        info: NodeInfo,
        blockchain: Blockchain,
        shared_api_state: SharedNodeState,
        node_channel: ExternalMessageSender,
    ) -> SystemApi {
        SystemApi {
            info,
            blockchain,
            node_channel,
            shared_api_state,
        }
    }

    fn peers_info(&self) -> PeersInfo {
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

        PeersInfo {
            incoming_connections: self.shared_api_state.incoming_connections(),
            outgoing_connections,
        }
    }

    fn handle_peers_info(self, router: &mut Router) {
        let peers_info = move |_: &mut Request| -> IronResult<Response> {
            let info = self.peers_info();
            self.ok_response(&serde_json::to_value(info).unwrap())
        };

        router.get("/v1/peers", peers_info, "peers_info");
    }

    fn handle_peer_add(self, router: &mut Router) {
        let peer_add = move |request: &mut Request| -> IronResult<Response> {
            let address: SocketAddr = self.required_param(request, "ip")?;
            self.node_channel.peer_add(address).map_err(ApiError::from)?;
            self.ok_response(&serde_json::to_value("Ok").unwrap())
        };

        router.post("/v1/peers", peer_add, "peer_add");
    }

    fn handle_network(self, router: &mut Router) {
        let network = move |_: &mut Request| -> IronResult<Response> {
            let info = self.info.clone();
            self.ok_response(&serde_json::to_value(info).unwrap())
        };

        router.get("/v1/network", network, "network_info");
    }

    fn handle_is_consensus_enabled(self, router: &mut Router) {
        let consensus_enabled_info = move |_: &mut Request| -> IronResult<Response> {
            let info = self.shared_api_state.is_enabled();
            self.ok_response(&serde_json::to_value(info).unwrap())
        };

        router.get(
            "/v1/consensus_enabled",
            consensus_enabled_info,
            "consensus_enabled_info",
        );
    }

    fn handle_set_consensus_enabled(self, router: &mut Router) {
        let consensus_enabled_set = move |request: &mut Request| -> IronResult<Response> {
            let enabled: bool = self.required_param(request, "enabled")?;
            self.node_channel.send_enable_consensus(enabled).map_err(
                ApiError::from,
            )?;
            self.ok_response(&serde_json::to_value("Ok").unwrap())
        };

        router.post(
            "/v1/consensus_enabled",
            consensus_enabled_set,
            "consensus_enabled_set",
        );
    }

    fn handle_shutdown(self, router: &mut Router) {
        let shutdown = move |_: &mut Request| -> IronResult<Response> {
            self.node_channel.send_shutdown().map_err(ApiError::from)?;
            self.ok_response(&serde_json::to_value("Ok").unwrap())
        };

        router.post("/v1/shutdown", shutdown, "shutdown");
    }
}

impl Api for SystemApi {
    fn wire(&self, router: &mut Router) {
        self.clone().handle_peers_info(router);
        self.clone().handle_peer_add(router);
        self.clone().handle_network(router);
        self.clone().handle_is_consensus_enabled(router);
        self.clone().handle_set_consensus_enabled(router);
        self.clone().handle_shutdown(router);
    }
}
