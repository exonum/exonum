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
//! - [Get node info](#get-node-info)
//! - [Get node statistics](#get-node-statistics)
//! - [Add peer](#add-peer)
//! - [Change consensus status](#change-consensus-status)
//! - [Node shutdown](#node-shutdown)
//!
//! # Get Node Info
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/info` |
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
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let info: NodeInfo = api.private(ApiKind::System).get("v1/info").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Get Node Statistics
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/stats` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | [`NodeStats`] |
//!
//! Returns the statistics of the current node.
//!
//! [`PeersInfo`]: struct.NodeStats.html
//!
//! ```
//! use exonum_system_api::{private::NodeStats, SystemApiPlugin};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let info: NodeStats = api.private(ApiKind::System).get("v1/stats").await?;
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
//! After adding a new peer the node config file will be rewritten.
//!
//! [`ConnectInfo`]: https://docs.rs/exonum-node/latest/exonum_node/struct.ConnectInfo.html
//!
//! ```
//! use exonum_node::ConnectInfo;
//! use exonum_system_api::SystemApiPlugin;
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
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
//!     .post("v1/peers")
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Change Consensus Status
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/consensus_status` |
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
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let enabled = true;
//! let query = ConsensusEnabledQuery::new(enabled);
//! api.private(ApiKind::System)
//!     .query(&query)
//!     .post("v1/consensus_status")
//!     .await?;
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
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! api.private(ApiKind::System)
//!     .post::<()>("v1/shutdown")
//!     .await?;
//! # Ok(())
//! # }
//! ```
// limitations under the License.

use exonum::{
    blockchain::{ApiSender, Blockchain, Schema},
    crypto::PublicKey,
    helpers::{exonum_version, os_info, rust_version},
};
use exonum_api::{self as api, ApiBackend, ApiScope};
use exonum_node::{ConnectInfo, ExternalMessage, SharedNodeState};
use futures::{future, prelude::*};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::SystemTime};

/// Information about the current state of the node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NodeStats {
    /// Height of the blockchain.
    pub height: u64,
    /// Total number of uncommitted transactions stored in persistent pool.
    pub tx_pool_size: u64,
    /// Total number of transactions in the blockchain.
    pub tx_count: u64,
    /// Size of the transaction cache.
    pub tx_cache_size: usize,
    /// Work duration of the node in seconds.
    pub uptime: u64,
}

/// Consensus status of the current node.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConsensusStatus {
    /// Consensus disabled on this node.
    Disabled,
    /// Consensus enabled on this node.
    Enabled,
    /// Consensus enabled and the node has enough connected peers.
    Active,
}

/// Type of the network connection.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConnectDirection {
    /// Incoming connection.
    Incoming,
    /// Outgoing connection.
    Outgoing,
}

/// Info about connected peer.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ConnectedPeerInfo {
    /// Address of the peer.
    pub address: String,
    /// Consensus public key of the peer.
    pub public_key: PublicKey,
    /// Connect direction.
    pub direction: ConnectDirection,
}

impl ConnectedPeerInfo {
    fn new(connect_info: &ConnectInfo, direction: ConnectDirection) -> Self {
        Self {
            address: connect_info.address.to_owned(),
            public_key: connect_info.public_key,
            direction,
        }
    }
}

/// Short information about the current node.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[non_exhaustive]
pub struct NodeInfo {
    /// Consensus status.
    pub consensus_status: ConsensusStatus,
    /// List of connected peers.
    pub connected_peers: Vec<ConnectedPeerInfo>,
    /// Version of the `exonum` crate.
    pub exonum_version: Version,
    /// Rust version.
    pub rust_version: Version,
    /// OS info.
    pub os_info: String,
}

/// Query for setting consensus enabled or disabled.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[non_exhaustive]
pub struct ConsensusEnabledQuery {
    /// Denotes if consensus should be enabled or disabled.
    pub enabled: bool,
}

impl ConsensusEnabledQuery {
    /// Creates a new consensus switch query.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

/// Private system API.
#[derive(Clone, Debug)]
pub(super) struct SystemApi {
    blockchain: Blockchain,
    shared_api_state: SharedNodeState,
    sender: ApiSender<ExternalMessage>,
    start_time: SystemTime,
}

impl SystemApi {
    /// Create a new `private::SystemApi` instance.
    pub fn new(
        blockchain: Blockchain,
        sender: ApiSender<ExternalMessage>,
        shared_api_state: SharedNodeState,
    ) -> Self {
        Self {
            blockchain,
            sender,
            shared_api_state,
            start_time: SystemTime::now(),
        }
    }

    /// Add private system API endpoints to the corresponding scope.
    pub fn wire(self, api_scope: &mut ApiScope) -> &mut ApiScope {
        self.handle_info("v1/info", api_scope)
            .handle_stats("v1/stats", api_scope)
            .handle_peers("v1/peers", api_scope)
            .handle_consensus_status("v1/consensus_status", api_scope)
            .handle_shutdown("v1/shutdown", api_scope);
        api_scope
    }

    fn handle_info(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let shared_api_state = self.shared_api_state.clone();
        api_scope.endpoint(name, move |_query: ()| {
            let mut connected_peers = Vec::new();

            for connect_info in shared_api_state.outgoing_connections() {
                connected_peers.push(ConnectedPeerInfo::new(
                    &connect_info,
                    ConnectDirection::Outgoing,
                ));
            }

            for connect_info in shared_api_state.incoming_connections() {
                connected_peers.push(ConnectedPeerInfo::new(
                    &connect_info,
                    ConnectDirection::Incoming,
                ));
            }

            let info = NodeInfo {
                consensus_status: Self::get_consensus_status(&shared_api_state),
                connected_peers,
                exonum_version: exonum_version().unwrap_or_else(|| Version::new(0, 0, 0)),
                rust_version: rust_version().unwrap_or_else(|| Version::new(0, 0, 0)),
                os_info: os_info(),
            };

            future::ok(info)
        });
        self
    }

    fn handle_stats(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let this = self.clone();
        api_scope.endpoint(name, move |_query: ()| {
            let snapshot = this.blockchain.snapshot();
            let schema = Schema::new(&snapshot);
            let uptime = SystemTime::now()
                .duration_since(this.start_time)
                .unwrap_or_default()
                .as_secs();
            let stats = NodeStats {
                height: schema.height().into(),
                tx_pool_size: schema.transactions_pool_len(),
                tx_count: schema.transactions_len(),
                tx_cache_size: this.shared_api_state.tx_cache_size(),
                uptime,
            };

            future::ok(stats)
        });
        self
    }

    fn handle_peers(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let sender = self.sender.clone();
        api_scope.endpoint_mut(name, move |connect_info: ConnectInfo| {
            let mut sender = sender.clone();
            async move {
                sender
                    .send_message(ExternalMessage::PeerAdd(connect_info))
                    .await
                    .map_err(|e| api::Error::internal(e).title("Failed to add peer"))
            }
        });
        self
    }

    fn handle_consensus_status(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let sender = self.sender.clone();
        api_scope.endpoint_mut(name, move |query: ConsensusEnabledQuery| {
            let mut sender = sender.clone();
            async move {
                sender
                    .send_message(ExternalMessage::Enable(query.enabled))
                    .await
                    .map_err(|e| api::Error::internal(e).title("Failed to set consensus enabled"))
            }
        });
        self
    }

    fn handle_shutdown(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        // These backend-dependent uses are needed to provide realization of the support of empty
        // request which is not easy in the generic approach, so it will be harder to misuse
        // those features (and as a result get a completely backend-dependent code).
        use actix_web::HttpResponse;
        use exonum_api::backends::actix::{RawHandler, RequestHandler};

        let sender = self.sender.clone();
        let index = move |_, _| {
            let mut sender = sender.clone();
            async move {
                sender
                    .send_message(ExternalMessage::Shutdown)
                    .await
                    .map(|_| HttpResponse::Ok().json(()))
                    .map_err(|e| {
                        api::Error::internal(e)
                            .title("Failed to handle shutdown")
                            .into()
                    })
            }
            .boxed_local()
        };

        let handler = RequestHandler {
            name: name.to_owned(),
            method: actix_web::http::Method::POST,
            inner: Arc::new(index) as Arc<RawHandler>,
        };
        api_scope.web_backend().raw_handler(handler);

        self
    }

    fn get_consensus_status(state: &SharedNodeState) -> ConsensusStatus {
        if state.is_enabled() {
            if state.consensus_status() {
                ConsensusStatus::Active
            } else {
                ConsensusStatus::Enabled
            }
        } else {
            ConsensusStatus::Disabled
        }
    }
}
