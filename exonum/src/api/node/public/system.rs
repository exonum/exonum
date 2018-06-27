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

//! Public system API.

use api::{ServiceApiScope, ServiceApiState};
use blockchain::{Schema, SharedNodeState};
use helpers::user_agent;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
/// Information about the current state of the Node memory pool.
pub struct MemPoolInfo {
    /// Total number of uncommitted transactions.
    pub size: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
/// Information about whether the node is connected to other peers.
pub struct HealthCheckInfo {
    /// Indicates whether the node is connected to the other peers.
    pub connectivity: bool,
}

/// Shows the possibility to achieve the consensus between validators
/// in the current state.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ConsensusStatusInfo {
    /// Consensus status: true - if consensus is achieved, false otherwise.
    pub status: bool,
}

/// Public system API.
#[derive(Clone, Debug)]
pub struct SystemApi {
    shared_api_state: SharedNodeState,
}

impl SystemApi {
    /// Creates a new `public::SystemApi` instance.
    pub fn new(shared_api_state: SharedNodeState) -> Self {
        SystemApi { shared_api_state }
    }

    fn handle_mempool_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        api_scope.endpoint(name, move |state: &ServiceApiState, _query: ()| {
            let snapshot = state.snapshot();
            let schema = Schema::new(&snapshot);
            Ok(MemPoolInfo {
                size: schema.transactions_pool_len(),
            })
        });
        self
    }

    fn handle_healthcheck_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_state: &ServiceApiState, _query: ()| {
            Ok(HealthCheckInfo {
                connectivity: !self.shared_api_state.peers_info().is_empty(),
            })
        });
        self_
    }

    fn handle_user_agent_info(self, name: &'static str, api_scope: &mut ServiceApiScope) -> Self {
        api_scope.endpoint(name, move |_state: &ServiceApiState, _query: ()| {
            Ok(user_agent::get())
        });
        self
    }

    fn handle_consensus_status_info(
        self,
        name: &'static str,
        api_scope: &mut ServiceApiScope,
    ) -> Self {
        let self_ = self.clone();
        api_scope.endpoint(name, move |_state: &ServiceApiState, _query: ()| {
            Ok(ConsensusStatusInfo {
                status: self.shared_api_state.consensus_status(),
            })
        });
        self_
    }

    /// Adds public system API endpoints to the corresponding scope.
    pub fn wire(self, api_scope: &mut ServiceApiScope) -> &mut ServiceApiScope {
        self.handle_mempool_info("v1/mempool", api_scope)
            .handle_healthcheck_info("v1/healthcheck", api_scope)
            .handle_user_agent_info("v1/user_agent", api_scope)
            .handle_consensus_status_info("v1/consensus_status", api_scope);
        api_scope
    }
}
