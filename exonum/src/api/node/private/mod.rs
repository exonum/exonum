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

//! Private part of the Exonum REST API.
//!
//! Private API includes requests that are available only to the blockchain
//! administrators, e.g. view the list of services on the current node.

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::{
    api::{node::SharedNodeState, ApiBackend, ApiScope, Error as ApiError},
    crypto::PublicKey,
    node::{ApiSender, ConnectInfo, ExternalMessage},
    runtime::InstanceId,
};

/// Short information about the service.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ServiceInfo {
    /// Service name.
    pub name: String,
    /// Service identifier for the database schema and service messages.
    pub id: InstanceId,
}

/// Short information about the current node.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NodeInfo {
    /// Version of the `exonum` crate.
    pub core_version: Option<String>,
}

impl NodeInfo {
    /// Creates new `NodeInfo` from services list.
    pub fn new() -> Self {
        let core_version = option_env!("CARGO_PKG_VERSION").map(ToOwned::to_owned);
        Self { core_version }
    }
}

impl Default for NodeInfo {
    fn default() -> Self {
        Self::new()
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
    incoming_connections: Vec<ConnectInfo>,
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
    sender: ApiSender,
}

impl SystemApi {
    /// Create a new `private::SystemApi` instance.
    pub fn new(sender: ApiSender, info: NodeInfo, shared_api_state: SharedNodeState) -> Self {
        Self {
            sender,
            info,
            shared_api_state,
        }
    }

    /// Add private system API endpoints to the corresponding scope.
    pub fn wire(self, api_scope: &mut ApiScope) -> &mut ApiScope {
        self.handle_peers_info("v1/peers", api_scope)
            .handle_peer_add("v1/peers", api_scope)
            .handle_network_info("v1/network", api_scope)
            .handle_is_consensus_enabled("v1/consensus_enabled", api_scope)
            .handle_set_consensus_enabled("v1/consensus_enabled", api_scope)
            .handle_shutdown("v1/shutdown", api_scope);
        api_scope
    }

    fn handle_peers_info(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_query: ()| {
            let mut outgoing_connections: HashMap<SocketAddr, IncomingConnection> = HashMap::new();

            for connect_info in self.shared_api_state.outgoing_connections() {
                outgoing_connections.insert(
                    connect_info.address.parse().unwrap(),
                    IncomingConnection {
                        public_key: Some(connect_info.public_key),
                        state: Default::default(),
                    },
                );
            }

            for (s, delay) in self.shared_api_state.reconnects_timeout() {
                outgoing_connections
                    .entry(s)
                    .or_insert_with(Default::default)
                    .state = IncomingConnectionState::Reconnect(ReconnectInfo { delay });
            }

            Ok(PeersInfo {
                incoming_connections: self.shared_api_state.incoming_connections(),
                outgoing_connections,
            })
        });
        self_
    }

    fn handle_peer_add(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint_mut(
            name,
            move |connect_info: ConnectInfo| -> Result<(), ApiError> {
                self.sender.peer_add(connect_info).map_err(ApiError::from)
            },
        );
        self_
    }

    fn handle_network_info(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_query: ()| Ok(self.info.clone()));
        self_
    }

    fn handle_is_consensus_enabled(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_query: ()| {
            Ok(self.shared_api_state.is_enabled())
        });
        self_
    }

    fn handle_set_consensus_enabled(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint_mut(
            name,
            move |query: ConsensusEnabledQuery| -> Result<(), ApiError> {
                self.sender
                    .send_external_message(ExternalMessage::Enable(query.enabled))
                    .map_err(ApiError::from)
            },
        );
        self_
    }

    fn handle_shutdown(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        // These uses need to provide realisation of the support of empty request
        // which is not easy in the generic approach, so it will be harder to misuse
        // those features (and as a result get a completely backend-dependent code).
        use crate::api::backends::actix::{FutureResponse, RawHandler, RequestHandler};
        use actix_web::{HttpRequest, HttpResponse};
        use futures::IntoFuture;

        let self_ = self.clone();

        let handler = move || -> Result<HttpResponse, ApiError> {
            self.sender
                .send_external_message(ExternalMessage::Shutdown)
                .map_err(ApiError::from)?;

            let ok_response = HttpResponse::Ok().json(());
            Ok(ok_response)
        };

        let index = move |_request: HttpRequest| -> FutureResponse {
            let future = handler().map_err(From::from).into_future();
            Box::new(future)
        };

        let handler = RequestHandler {
            name: name.to_owned(),
            method: actix_web::http::Method::POST,
            inner: Arc::from(index) as Arc<RawHandler>,
        };

        api_scope.web_backend().raw_handler(handler);

        self_
    }
}
