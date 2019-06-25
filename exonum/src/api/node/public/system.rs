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

use crate::api::{Error as ApiError, ServiceApiScope, ServiceApiState};
use crate::blockchain::{Schema, SharedNodeState, GenesisConfig};
use crate::helpers::user_agent;
use crate::crypto::PublicKey;
use exonum_merkledb::DbOptions;
use crate::node::{NodeConfig, MemoryPoolConfig, ConnectListConfig, AuditorConfig};
use std::path::PathBuf;
use crate::events::NetworkConfiguration;
use std::collections::btree_map::BTreeMap;
use toml::Value;

/// Information about the current state of the node memory pool.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct StatsInfo {
    /// Total number of uncommitted transactions.
    pub tx_pool_size: u64,
    /// Total number of transactions in the blockchain.
    pub tx_count: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ServiceInfo {
    name: String,
    id: u16,
}

/// Services info response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServicesResponse {
    services: Vec<ServiceInfo>,
}

/// Information about service public key.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct KeyInfo {
    /// Public key.
    pub pub_key: PublicKey,
}

/// Shared configuration
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SharedConfiguration {
    /// Initial config that will be written in the first block.
    pub genesis: GenesisConfig,
    /// Remote Network address used by this node.
    pub external_address: String,
    /// Network configuration.
    pub network: NetworkConfiguration,
    /// Memory pool configuration.
    pub mempool: MemoryPoolConfig,
    /// Additional config, usable for services.
    #[serde(default)]
    pub services_configs: BTreeMap<String, Value>,
    /// Optional database configuration.
    #[serde(default)]
    pub database: DbOptions,
    /// Node's ConnectList.
    pub connect_list: ConnectListConfig,
    /// Transaction Verification Thread Pool size.
    pub thread_pool_size: Option<u8>,
    /// Auditor configuration.
    pub auditor: AuditorConfig,
    /// Consensus public key.
    pub consensus_public_key: PublicKey,
}

impl SharedConfiguration {
    /// Create new shared configuration.
    pub fn new(config: NodeConfig<PathBuf>) -> SharedConfiguration {
        SharedConfiguration {
            genesis: config.genesis,
            external_address: config.external_address,
            network: config.network,
            mempool: config.mempool,
            services_configs: config.services_configs,
            database: config.database,
            connect_list: config.connect_list,
            thread_pool_size: config.thread_pool_size,
            auditor: config.auditor,
            consensus_public_key: config.consensus_public_key,
        }
    }
}

/// Public system API.
#[derive(Clone, Debug)]
pub struct SystemApi {
    shared_api_state: SharedNodeState,
}

impl SystemApi {
    /// Creates a new `public::SystemApi` instance.
    pub fn new(shared_api_state: SharedNodeState) -> Self {
        Self { shared_api_state }
    }

    fn handle_stats_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        api_scope.endpoint(name, move |state: &ServiceApiState, _query: ()| {
            let snapshot = state.snapshot();
            let schema = Schema::new(&snapshot);
            Ok(StatsInfo {
                tx_pool_size: schema.transactions_pool_len(),
                tx_count: schema.transactions_len(),
            })
        });
        self
    }

    fn handle_user_agent_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        api_scope.endpoint(name, move |_state: &ServiceApiState, _query: ()| {
            Ok(user_agent::get())
        });
        self
    }

    fn handle_healthcheck_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_state: &ServiceApiState, _query: ()| {
            Ok(HealthCheckInfo {
                consensus_status: self.get_consensus_status(),
                connected_peers: self.get_number_of_connected_peers(),
            })
        });
        self_
    }

    fn handle_list_services_info(
        self,
        name: &'static str,
        api_scope: &mut ServiceApiScope,
    ) -> Self {
        api_scope.endpoint(name, move |state: &ServiceApiState, _query: ()| {
            let blockchain = state.blockchain();
            let services = blockchain
                .service_map()
                .iter()
                .map(|(&id, service)| ServiceInfo {
                    name: service.service_name().to_string(),
                    id,
                })
                .collect::<Vec<_>>();
            Ok(ServicesResponse { services })
        });
        self
    }

    fn get_number_of_connected_peers(&self) -> usize {
        let in_conn = self.shared_api_state.incoming_connections().len();
        let out_conn = self.shared_api_state.outgoing_connections().len();
        // We sum incoming and outgoing connections here because we keep only one connection
        // between nodes. A connection could be incoming or outgoing but only one.
        in_conn + out_conn
    }

    fn get_consensus_status(&self) -> ConsensusStatus {
        if self.shared_api_state.is_enabled() {
            if self.shared_api_state.consensus_status() {
                ConsensusStatus::Active
            } else {
                ConsensusStatus::Enabled
            }
        } else {
            ConsensusStatus::Disabled
        }
    }

    fn handle_service_key_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        api_scope.endpoint(name, move |state: &ServiceApiState, _query: ()| {
            Ok(KeyInfo { pub_key: state.public_key().clone() })
        });
        self
    }

    fn handle_remote_config_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        let _self = self.clone();
        api_scope.endpoint(name, move |_state: &ServiceApiState, query: KeyInfo| {
            if !self.shared_api_state.has_peer(&query.pub_key) {
                return Err(ApiError::NotFound("Peer with this public key not found".to_owned()));
            }

            self.shared_api_state.load_configuration()
                .map(SharedConfiguration::new)
                .ok_or(ApiError::NotFound("Node configuration not found".to_owned()))
        });

        _self
    }

    /// Adds public system API endpoints to the corresponding scope.
    pub fn wire(self, api_scope: &mut ServiceApiScope) -> &mut ServiceApiScope {
        self.handle_stats_info("v1/stats", api_scope)
            .handle_healthcheck_info("v1/healthcheck", api_scope)
            .handle_user_agent_info("v1/user_agent", api_scope)
            .handle_list_services_info("v1/services", api_scope)
            .handle_user_agent_info("v1/user_agent", api_scope)
            .handle_service_key_info("v1/service_key", api_scope)
            .handle_remote_config_info("v1/remote_config", api_scope);
        api_scope
    }
}
