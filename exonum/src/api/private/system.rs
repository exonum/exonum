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

use std::net::SocketAddr;
use std::collections::HashMap;

use router::Router;
use iron::prelude::*;

use crypto::PublicKey;
use node::{ExternalMessage, ApiSender};
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
    node_channel: ApiSender,
}

impl SystemApi {
    /// Creates a new `public::SystemApi` instance.
    pub fn new(
        info: NodeInfo,
        blockchain: Blockchain,
        shared_api_state: SharedNodeState,
        node_channel: ApiSender,
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

    fn network_info(&self) -> NodeInfo {
        self.info.clone()
    }

    fn is_consensus_enabled(&self) -> bool {
        self.shared_api_state.is_enabled()
    }

    fn set_consensus_enabled(&self, enabled: bool) -> Result<(), ApiError> {
        let message = ExternalMessage::Enable(enabled);
        self.node_channel.send_external_message(message)?;
        Ok(())
    }
}

impl Api for SystemApi {
    fn wire(&self, router: &mut Router) {

        let self_ = self.clone();
        let peer_add = move |req: &mut Request| -> IronResult<Response> {
            let addr: SocketAddr = self_.required_param(req, "ip")?;
            self_.node_channel.peer_add(addr).map_err(ApiError::from)?;
            self_.ok_response(&::serde_json::to_value("Ok").unwrap())
        };

        let self_ = self.clone();
        let peers_info = move |_: &mut Request| -> IronResult<Response> {
            let info = self_.peers_info();
            self_.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let network = move |_: &mut Request| -> IronResult<Response> {
            let info = self_.network_info();
            self_.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let consensus_enabled_info = move |_: &mut Request| -> IronResult<Response> {
            let info = self_.is_consensus_enabled();
            self_.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let consensus_enabled_set = move |req: &mut Request| -> IronResult<Response> {
            let enabled: bool = self_.required_param(req, "enabled")?;
            self_.set_consensus_enabled(enabled)?;
            self_.ok_response(&::serde_json::to_value("Ok").unwrap())
        };

        router.get("/v1/peers", peers_info, "peers_info");
        router.post("/v1/peers", peer_add, "peer_add");
        router.get("/v1/network", network, "network_info");
        router.get(
            "/v1/consensus_enabled",
            consensus_enabled_info,
            "consensus_enabled_info",
        );
        router.post(
            "/v1/consensus_enabled",
            consensus_enabled_set,
            "consensus_enabled_set",
        );
    }
}
