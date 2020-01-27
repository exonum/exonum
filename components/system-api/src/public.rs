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

//! Public part of the node REST API.
//!
//! Public API includes universally available endpoints, e.g., allowing to view
//! the list of services on the current node.
//!
//! # Table of Contents
//!
//! - [Network statistics](#network-statistics)
//! - [Node health info](#node-health-info)
//! - [User agent](#user-agent)
//! - [Available services](#available-services)
//!
//! # Network Statistics
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/stats` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | [`StatsInfo`] |
//!
//! Returns information about the current state of the node memory pool.
//!
//! [`StatsInfo`]: struct.StatsInfo.html
//!
//! ```
//! use exonum_system_api::{public::StatsInfo, SystemApiPlugin};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let stats: StatsInfo = api.public(ApiKind::System).get("v1/stats")?;
//! # Ok(())
//! # }
//! ```
//!
//! # Node Health Info
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/healthcheck` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | [`HealthCheckInfo`] |
//!
//! Returns information about whether the node is connected to other peers and
//! its consensus status.
//!
//! [`HealthCheckInfo`]: struct.HealthCheckInfo.html
//!
//! ```
//! use exonum_system_api::{public::HealthCheckInfo, SystemApiPlugin};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let info: HealthCheckInfo = api.public(ApiKind::System).get("v1/healthcheck")?;
//! # Ok(())
//! # }
//! ```
//!
//! # User Agent
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/user_agent` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | `String` |
//!
//! Returns an user agent of the node.
//!
//! User agent includes versions of Exonum and the Rust compiler and the OS info,
//! all separated by slashes.
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
//! let user_agent: String = api.public(ApiKind::System).get("v1/user_agent")?;
//!
//! let components: Vec<_> = user_agent.split('/').collect();
//! assert_eq!(components.len(), 3);
//! let exonum_version = components[0];
//! let rust_version = components[1];
//! let os_version = components[2];
//! # Ok(())
//! # }
//! ```
//!
//! # Available Services
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/system/v1/services` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | [`DispatcherInfo`] |
//!
//! Returns information about services available in the network.
//!
//! [`DispatcherInfo`]: struct.DispatcherInfo.html
//!
//! ```
//! use exonum_system_api::{public::DispatcherInfo, SystemApiPlugin};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let user_agent: DispatcherInfo = api.public(ApiKind::System).get("v1/services")?;
//! # Ok(())
//! # }
//! ```

use exonum::{
    blockchain::{Blockchain, Schema},
    helpers::user_agent,
    merkledb::access::AsReadonly,
    runtime::{ArtifactId, DispatcherSchema, InstanceState, SnapshotExt},
};
use exonum_api::ApiScope;
use exonum_node::SharedNodeState;
use serde_derive::{Deserialize, Serialize};

/// Information about the current state of the node memory pool.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct StatsInfo {
    /// Total number of uncommitted transactions stored in persistent pool.
    pub tx_pool_size: u64,
    /// Total number of transactions in the blockchain.
    pub tx_count: u64,
    /// Size of the transaction cache.
    pub tx_cache_size: usize,
}

/// Information about whether it is possible to achieve the consensus between
/// validators in the current state.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub enum ConsensusStatus {
    /// Consensus disabled on this node.
    Disabled,
    /// Consensus enabled on this node.
    Enabled,
    /// Consensus enabled and the node has enough connected peers.
    Active,
}

/// Information about whether the node is connected to other peers and
/// its consensus status.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct HealthCheckInfo {
    /// Consensus status.
    pub consensus_status: ConsensusStatus,
    /// The number of connected peers to the node.
    pub connected_peers: usize,
}

/// Services info response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DispatcherInfo {
    /// List of deployed artifacts.
    pub artifacts: Vec<ArtifactId>,
    /// List of services.
    pub services: Vec<InstanceState>,
}

impl DispatcherInfo {
    /// Loads dispatcher information from database.
    fn load<T: AsReadonly>(schema: &DispatcherSchema<T>) -> Self {
        Self {
            artifacts: schema.service_artifacts().keys().collect(),
            services: schema.service_instances().values().collect(),
        }
    }
}

/// Public system API.
#[derive(Clone, Debug)]
pub(super) struct SystemApi {
    blockchain: Blockchain,
    node_state: SharedNodeState,
}

impl SystemApi {
    /// Create a new `public::SystemApi` instance.
    pub fn new(blockchain: Blockchain, node_state: SharedNodeState) -> Self {
        Self {
            blockchain,
            node_state,
        }
    }

    fn handle_stats_info(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_query: ()| {
            let snapshot = self.blockchain.snapshot();
            let schema = Schema::new(&snapshot);
            Ok(StatsInfo {
                tx_pool_size: schema.transactions_pool_len(),
                tx_count: schema.transactions_len(),
                tx_cache_size: self.node_state.tx_cache_size(),
            })
        });
        self_
    }

    fn handle_user_agent_info(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        api_scope.endpoint(name, move |_query: ()| Ok(user_agent()));
        self
    }

    fn handle_healthcheck_info(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_query: ()| {
            Ok(HealthCheckInfo {
                consensus_status: self.get_consensus_status(),
                connected_peers: self.get_number_of_connected_peers(),
            })
        });
        self_
    }

    fn handle_list_services_info(self, name: &'static str, api_scope: &mut ApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_query: ()| {
            let snapshot = self_.blockchain.snapshot();
            Ok(DispatcherInfo::load(&snapshot.for_dispatcher()))
        });
        self
    }

    fn get_number_of_connected_peers(&self) -> usize {
        let in_conn = self.node_state.incoming_connections().len();
        let out_conn = self.node_state.outgoing_connections().len();
        // Sum incoming and outgoing connections here to keep only one connection
        // between nodes. There can be only one connection - either incoming or outgoing one.
        in_conn + out_conn
    }

    fn get_consensus_status(&self) -> ConsensusStatus {
        if self.node_state.is_enabled() {
            if self.node_state.consensus_status() {
                ConsensusStatus::Active
            } else {
                ConsensusStatus::Enabled
            }
        } else {
            ConsensusStatus::Disabled
        }
    }

    /// Add public system API endpoints to the corresponding scope.
    pub fn wire(self, api_scope: &mut ApiScope) -> &mut ApiScope {
        self.handle_stats_info("v1/stats", api_scope)
            .handle_healthcheck_info("v1/healthcheck", api_scope)
            .handle_user_agent_info("v1/user_agent", api_scope)
            .handle_list_services_info("v1/services", api_scope);
        api_scope
    }
}
