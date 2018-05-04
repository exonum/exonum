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

//! Unsafe Exonum api that provides functionality for emergency cases.

use router::Router;
use iron::prelude::*;

use std::process;

use blockchain::{Blockchain, Schema};
use api::{Api, ApiError};

/// Private unsafe API.
#[derive(Clone, Debug)]
pub struct UnsafeApi {
    blockchain: Blockchain,
}

impl UnsafeApi {
    /// Creates a new `private::UnsafeApi` instance.
    pub fn new(blockchain: Blockchain) -> UnsafeApi {
        UnsafeApi { blockchain }
    }

    fn handle_unsafe_shutdown(self, router: &mut Router) {
        let shutdown_unsafe = move |_: &mut Request| -> IronResult<Response> {
            warn!("Received unsafe shutdown request");

            // Clear cache.
            let mut blockchain = self.blockchain.clone();
            let mut fork = blockchain.fork();
            {
                let mut schema = Schema::new(&mut fork);
                schema.consensus_messages_cache_mut().clear();
            }
            blockchain
                .merge_sync(fork.into_patch())
                .map_err(ApiError::Storage)?;

            // Desrtoy this process.
            process::exit(1);
        };

        router.post("/v1/shutdown", shutdown_unsafe, "unsafe_shutdown");
    }
}

impl Api for UnsafeApi {
    fn wire(&self, router: &mut Router) {
        self.clone().handle_unsafe_shutdown(router);
    }
}
