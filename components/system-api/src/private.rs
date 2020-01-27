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

//! Private part of the node REST API.
//!
//! Private API includes requests that are available only to the blockchain
//! administrators, e.g. shutting down the node.
//!
//! # Table of Contents
//!
//! - [List peers](#list-peers)
//! - [Add peer](#add-peer)
//! - [Get node info](#get-node-info)
//! - [Check if consensus is enabled](#check-if-consensus-is-enabled)
//! - [Change consensus status](#change-consensus-status)
//! - [Node shutdown](#node-shutdown)
//!
//! # List Peers
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/peers` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | [`PeersInfo`] |
//!
//! Returns the list of incoming and outgoing connections for current node.
//!
//! [`PeersInfo`]: struct.PeersInfo.html
//!
//! ```
//! use exonum_system_api::{private::PeersInfo, SystemApiPlugin};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let info: PeersInfo = api.private(ApiKind::System).get("v1/peers")?;
//! # Ok(())
//! # }
//! ```
//!
//! # Add Peer
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/peers` |
//! | Method      | POST   |
//! | Query type  | [`ConnectInfo`] |
//! | Return type | - |
//!
//! Adds a peer to the Exonum node. Node will attempt to connect to this peer.
//!
//! [`ConnectInfo`]: https://docs.rs/exonum-node/latest/exonum_node/struct.ConnectInfo.html
//!
//! ```
//! use exonum_node::ConnectInfo;
//! use exonum_system_api::SystemApiPlugin;
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! # let address = "127.0.0.1:8080".to_owned();
//! # let public_key = Default::default();
//! // Obtaining address and public key of target node skipped...
//! let connect_info = ConnectInfo {
//!     address,
//!     public_key,
//! };
//!
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! api.private(ApiKind::System)
//!     .query(&connect_info)
//!     .post("v1/peers")?;
//! # Ok(())
//! # }
//! ```
//!
//! # Get Node Info
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/network` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | [`NodeInfo`] |
//!
//! Obtains information about node.
//!
//! [`NodeInfo`]: struct.NodeInfo.html
//!
//! ```
//! use exonum_system_api::{private::NodeInfo, SystemApiPlugin};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let info: NodeInfo = api.private(ApiKind::System).get("v1/network")?;
//! # Ok(())
//! # }
//! ```
//!
//! # Check if Consensus is Enabled
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/consensus_enabled` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | `bool` |
//!
//! Returns `true` if consensus is enabled on the node, and `false` otherwise.
//!
//! Note the difference between `consensus_enabled` and
//! [public `healthcheck` endpoint](../public/index.html#node-health-info):
//!
//! This endpoint only reports the setting value (should the node participate in consensus
//! or not), while `healthcheck` provides information about consensus status (since even
//! with consensus setting turned on, node may not participate in consensus due to lack of
//! peers).
//!
//! ```
//! use exonum_system_api::SystemApiPlugin;
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let consensus_enabled: bool = api.private(ApiKind::System).get("v1/consensus_enabled")?;
//! # Ok(())
//! # }
//! ```
//!
//! # Change Consensus Status
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/consensus_enabled` |
//! | Method      | POST   |
//! | Query type  | [`ConsensusEnabledQuery`] |
//! | Return type | - |
//!
//! Enables or disables consensus on the node.
//!
//! [`ConsensusEnabledQuery`]: struct.ConsensusEnabledQuery.html
//!
//! ```
//! use exonum_system_api::{private::ConsensusEnabledQuery, SystemApiPlugin};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let enabled = true;
//! let query = ConsensusEnabledQuery { enabled };
//! api.private(ApiKind::System)
//!     .query(&query)
//!     .post("v1/consensus_enabled")?;
//! # Ok(())
//! # }
//! ```
//!
//! # Node Shutdown
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/shutdown` |
//! | Method      | POST   |
//! | Query type  | - |
//! | Return type | - |
//!
//! Shuts down the node.
//!
//! ```
//! use exonum_system_api::SystemApiPlugin;
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! api.private(ApiKind::System).post::<()>("v1/shutdown")?;
//! # Ok(())
//! # }
//! ```

use exonum::{blockchain::ApiSender, crypto::PublicKey, runtime::InstanceId};
use exonum_api::{self as api, ApiBackend, ApiScope};
use exonum_node::{ConnectInfo, ExternalMessage, SharedNodeState};
use futures::Future;
use serde::{Deserialize, Serialize};

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

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

/// Information about the outgoing connection.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OutgoingConnection {
    /// Public key of target node, if any.
    pub public_key: Option<PublicKey>,
}

/// Information about connections of the node.
#[derive(Debug, Serialize, Deserialize)]
pub struct PeersInfo {
    /// Incoming connections.
    pub incoming_connections: Vec<ConnectInfo>,
    /// Outgoing connections.
    pub outgoing_connections: HashMap<SocketAddr, OutgoingConnection>,
}

/// Query for setting consensus enabled or disabled.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConsensusEnabledQuery {
    /// Denotes if consensus should be enabled or disabled.
    pub enabled: bool,
}

/// Private system API.
#[derive(Debug)]
pub(super) struct SystemApi {
    info: NodeInfo,
    shared_api_state: SharedNodeState,
    sender: ApiSender<ExternalMessage>,
}

impl SystemApi {
    /// Create a new `private::SystemApi` instance.
    pub fn new(sender: ApiSender<ExternalMessage>, shared_api_state: SharedNodeState) -> Self {
        Self {
            sender,
            info: NodeInfo::new(),
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
        let shared_api_state = self.shared_api_state.clone();
        api_scope.endpoint(name, move |_query: ()| -> api::Result<_> {
            let mut outgoing_connections: HashMap<SocketAddr, OutgoingConnection> = HashMap::new();

            for connect_info in shared_api_state.outgoing_connections() {
                outgoing_connections.insert(
                    connect_info.address.parse().unwrap(),
                    OutgoingConnection {
                        public_key: Some(connect_info.public_key),
                    },
                );
            }

            Ok(PeersInfo {
                incoming_connections: shared_api_state.incoming_connections(),
                outgoing_connections,
            })
        });
        self
    }

    fn handle_peer_add(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let sender = self.sender.clone();
        api_scope.endpoint_mut(
            name,
            move |connect_info: ConnectInfo| -> api::FutureResult<()> {
                let handler = sender
                    .send_message(ExternalMessage::PeerAdd(connect_info))
                    .map_err(|e| api::Error::internal(e).title("Failed to add peer"));
                Box::new(handler)
            },
        );
        self
    }

    fn handle_network_info(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let info = self.info.clone();
        api_scope.endpoint(name, move |_query: ()| -> api::Result<_> {
            Ok(info.clone())
        });
        self
    }

    fn handle_is_consensus_enabled(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let shared_api_state = self.shared_api_state.clone();
        api_scope.endpoint(name, move |_query: ()| -> api::Result<_> {
            Ok(shared_api_state.is_enabled())
        });
        self
    }

    fn handle_set_consensus_enabled(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let sender = self.sender.clone();
        api_scope.endpoint_mut(
            name,
            move |query: ConsensusEnabledQuery| -> api::FutureResult<()> {
                let handler = sender
                    .send_message(ExternalMessage::Enable(query.enabled))
                    .map_err(|e| api::Error::internal(e).title("Failed to set consensus enabled"));
                Box::new(handler)
            },
        );
        self
    }

    fn handle_shutdown(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        // These backend-dependent uses are needed to provide realization of the support of empty
        // request which is not easy in the generic approach, so it will be harder to misuse
        // those features (and as a result get a completely backend-dependent code).
        use actix_web::{HttpRequest, HttpResponse};
        use exonum_api::backends::actix::{FutureResponse, RawHandler, RequestHandler};

        let sender = self.sender.clone();
        let index = move |_: HttpRequest| -> FutureResponse {
            let handler = sender
                .send_message(ExternalMessage::Shutdown)
                .map(|()| HttpResponse::Ok().json(()))
                .map_err(|e| {
                    let e = api::Error::internal(e).title("Failed to handle shutdown");
                    actix_web::Error::from(e)
                });
            Box::new(handler)
        };

        let handler = RequestHandler {
            name: name.to_owned(),
            method: actix_web::http::Method::POST,
            inner: Arc::new(index) as Arc<RawHandler>,
        };
        api_scope.web_backend().raw_handler(handler);

        self
    }
}
