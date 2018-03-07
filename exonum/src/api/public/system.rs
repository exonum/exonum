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

use router::Router;
use iron::prelude::*;

use node::state::TxPool;
use blockchain::{Blockchain, SharedNodeState};
use api::Api;
use helpers::user_agent;

#[derive(Serialize, Deserialize, PartialEq)]
struct MemPoolInfo {
    pub size: usize,
}

#[doc(hidden)]
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct HealthCheckInfo {
    pub connectivity: bool,
}

/// Public system API.
#[derive(Clone, Debug)]
pub struct SystemApi {
    pool: TxPool,
    blockchain: Blockchain,
    shared_api_state: SharedNodeState,
}

impl SystemApi {
    /// Creates a new `private::SystemApi` instance.
    pub fn new(
        pool: TxPool,
        blockchain: Blockchain,
        shared_api_state: SharedNodeState,
    ) -> SystemApi {
        SystemApi {
            pool,
            blockchain,
            shared_api_state,
        }
    }

    fn mempool_info(self, router: &mut Router) {
        let mempool = move |_: &mut Request| -> IronResult<Response> {
            let info = MemPoolInfo { size: self.pool.read().expect("Expected read lock").len() };
            self.ok_response(&::serde_json::to_value(info).unwrap())
        };
        router.get("/v1/mempool", mempool, "mempool");
    }

    fn healthcheck_info(self, router: &mut Router) {
        let healthcheck = move |_: &mut Request| -> IronResult<Response> {
            let info =
                HealthCheckInfo { connectivity: !self.shared_api_state.peers_info().is_empty() };
            self.ok_response(&::serde_json::to_value(info).unwrap())
        };
        router.get("/v1/healthcheck", healthcheck, "healthcheck");
    }

    fn user_agent_info(self, router: &mut Router) {
        let user_agent = move |_: &mut Request| -> IronResult<Response> {
            let info = user_agent::get();
            self.ok_response(&::serde_json::to_value(info).unwrap())
        };
        router.get("/v1/user_agent", user_agent, "user_agent");
    }
}

impl Api for SystemApi {
    fn wire(&self, router: &mut Router) {
        self.clone().mempool_info(router);
        self.clone().healthcheck_info(router);
        self.clone().user_agent_info(router);
    }
}
