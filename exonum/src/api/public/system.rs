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

#[derive(Serialize)]
struct MemPoolInfo {
    size: usize,
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

    fn get_mempool_info(&self) -> MemPoolInfo {
        MemPoolInfo { size: self.pool.read().expect("Expected read lock").len() }
    }

    fn get_healthcheck_info(&self) -> HealthCheckInfo {
        HealthCheckInfo { connectivity: !self.shared_api_state.peers_info().is_empty() }
    }
}

impl Api for SystemApi {
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let mempool_info = move |_: &mut Request| -> IronResult<Response> {
            let info = self_.get_mempool_info();
            self_.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let healthcheck = move |_: &mut Request| {
            let info = self_.get_healthcheck_info();
            self_.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/mempool", mempool_info, "mempool");
        router.get("/v1/healthcheck", healthcheck, "healthcheck_info");
    }
}
