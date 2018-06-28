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

use iron::prelude::*;
use router::Router;
use serde_json;

use api::Api;
use blockchain::{Blockchain, Schema, SharedNodeState};
use helpers::user_agent;

#[derive(Serialize, Deserialize, PartialEq)]
struct MemPoolInfo {
    pub size: usize,
}

//#[doc(hidden)]
//#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
//pub struct HealthCheckInfo {
//    pub connectivity: bool,
//}

/// ConsensusStatusInfo shows the possibility to achieve the consensus between validators
/// in current state.
//#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
//pub struct ConsensusStatusInfo {
//    /// Consensus status: true - if consensus achieved, else - false.
//    pub status: bool,
//}

/// Contains amount of connected peers
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct PeersAmount {
    /// amount of connected peers
    pub amount: usize,
}

/// Shows connectivity status of the node
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub enum ConnectivityStatus {
    /// The node has no peers
    NotConnected,
    /// The node has amount of peers
    Connected(PeersAmount),
}

/// Shows status of the consensus
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub enum ConsensusStatus {
    /// Consensus disabled
    Disabled,
    /// Consensus enabled but not active
    Enabled,
    /// Consensus enabled and active
    Active,
}

/// Information about connectivity and consensus
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ConnectivityInfo {
    /// Consensus status
    pub consensus_status: ConsensusStatus,
    /// Connectivity status
    pub connectivity: ConnectivityStatus,
}

/// Public system API.
#[derive(Clone, Debug)]
pub struct SystemApi {
    blockchain: Blockchain,
    shared_api_state: SharedNodeState,
}

impl SystemApi {
    /// Creates a new `private::SystemApi` instance.
    pub fn new(blockchain: Blockchain, shared_api_state: SharedNodeState) -> SystemApi {
        SystemApi {
            blockchain,
            shared_api_state,
        }
    }

    fn mempool_info(self, router: &mut Router) {
        let mempool = move |_: &mut Request| -> IronResult<Response> {
            let snapshot = self.blockchain.snapshot();
            let schema = Schema::new(&snapshot);
            let info = MemPoolInfo {
                size: schema.transactions_pool_len(),
            };
            self.ok_response(&serde_json::to_value(info).unwrap())
        };
        router.get("/v1/mempool", mempool, "mempool");
    }

    //fn healthcheck_info(self, router: &mut Router) {
    //    let healthcheck = move |_: &mut Request| -> IronResult<Response> {
    //        let info = HealthCheckInfo {
    //            connectivity: !self.shared_api_state.peers_info().is_empty(),
    //        };
    //        self.ok_response(&serde_json::to_value(info).unwrap())
    //    };
    //    router.get("/v1/healthcheck", healthcheck, "healthcheck");
    //}

    fn user_agent_info(self, router: &mut Router) {
        let user_agent = move |_: &mut Request| -> IronResult<Response> {
            let info = user_agent::get();
            self.ok_response(&serde_json::to_value(info).unwrap())
        };
        router.get("/v1/user_agent", user_agent, "user_agent");
    }

    fn connectivity_status_info(self, router: &mut Router) {
        let connectivity_status = move |_: &mut Request| -> IronResult<Response> {
            let peers_info = self.shared_api_state.peers_info();

            let connectivity = if peers_info.is_empty() {
                ConnectivityStatus::NotConnected
            } else {
                ConnectivityStatus::Connected(PeersAmount {
                    amount: peers_info.len(),
                })
            };

            let consensus_status = if self.shared_api_state.is_enabled() {
                if self.shared_api_state.consensus_status() {
                    ConsensusStatus::Active
                } else {
                    ConsensusStatus::Enabled
                }
            } else {
                ConsensusStatus::Disabled
            };

            let info = ConnectivityInfo {
                consensus_status,
                connectivity,
            };

            self.ok_response(&serde_json::to_value(info).unwrap())
        };
        router.get(
            "/v1/connectivity_status",
            connectivity_status,
            "connectivity_status"
        );
    }
}

impl Api for SystemApi {
    fn wire(&self, router: &mut Router) {
        self.clone().mempool_info(router);
        //self.clone().healthcheck_info(router);
        self.clone().user_agent_info(router);
        self.clone().connectivity_status_info(router);
    }
}
