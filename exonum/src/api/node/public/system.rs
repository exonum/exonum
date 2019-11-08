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

//! Public system API.

use exonum_merkledb::IndexAccess;

use crate::{
    api::{node::SharedNodeState, ApiScope},
    blockchain::{Blockchain, Schema},
    helpers::user_agent,
    runtime::{ArtifactId, DispatcherSchema, InstanceSpec},
};

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
    pub services: Vec<InstanceSpec>,
}

impl DispatcherInfo {
    /// Loads dispatcher information from database.
    pub fn load(access: impl IndexAccess) -> Self {
        let schema = DispatcherSchema::new(access);
        Self {
            artifacts: schema
                .artifacts()
                .values()
                .map(|spec| spec.artifact)
                .collect(),
            services: schema.service_instances().values().collect(),
        }
    }
}

/// Protobuf sources query parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtoSourcesQuery {
    /// Artifact identifier, if specified, query returns the source files of the artifact,
    /// otherwise it returns source files of `exonum` itself.
    pub artifact: Option<String>,
}

/// Public system API.
#[derive(Clone, Debug)]
pub struct SystemApi {
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
        api_scope.endpoint(name, move |_query: ()| Ok(user_agent::get()));
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
            Ok(DispatcherInfo::load(&self_.blockchain.snapshot()))
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
