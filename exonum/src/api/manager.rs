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

//! Module responsible for actix web API management after new service is deployed.

use actix::prelude::*;
use actix_net::server::Server;
use actix_web::server::{HttpServer, StopServer};
use futures::{sync::mpsc, Future};

use std::{collections::HashMap, fmt, io, time::Duration};

use crate::api::{
    backends::actix::{create_app, ApiRuntimeConfig, SystemRuntimeConfig},
    ApiBuilder,
};

/// Actor responsible for API management.
pub struct ApiManager {
    runtime_config: SystemRuntimeConfig,
    api_runtime_addresses: HashMap<Addr<Server>, ApiRuntimeConfig>,
    user_endpoints: Vec<(String, ApiBuilder)>,
    endpoints_rx: Option<mpsc::Receiver<UpdateEndpoints>>,
}

impl fmt::Debug for ApiManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiManager")
            .field("runtime_config", &self.runtime_config)
            .finish()
    }
}

impl ApiManager {
    pub fn new(
        runtime_config: SystemRuntimeConfig,
        endpoints_rx: mpsc::Receiver<UpdateEndpoints>,
    ) -> Self {
        Self {
            runtime_config,
            api_runtime_addresses: HashMap::new(),
            user_endpoints: vec![],
            endpoints_rx: Some(endpoints_rx),
        }
    }

    fn start_api_servers(&mut self) {
        self.api_runtime_addresses = self
            .runtime_config
            .api_runtimes
            .iter()
            .cloned()
            .map(|runtime_config| {
                let server_address = self
                    .start_server(runtime_config.clone())
                    .expect("Failed to start API server");
                (server_address, runtime_config)
            })
            .collect();
    }

    fn start_server(&self, runtime_config: ApiRuntimeConfig) -> io::Result<Addr<Server>> {
        let access = runtime_config.access;
        let listen_address = runtime_config.listen_address;
        info!("Starting {} web api on {}", access, listen_address);

        let mut aggregator = self.runtime_config.api_aggregator.clone();
        aggregator.extend(self.user_endpoints.clone());
        HttpServer::new(move || create_app(&aggregator, runtime_config.clone()))
            .disable_signals()
            .bind(listen_address)
            .map(HttpServer::start)
    }

    fn initiate_restart(&mut self, manager: Addr<Self>) {
        info!("Restarting servers.");
        for (addr, config) in self.api_runtime_addresses.drain() {
            let manager = manager.clone();
            Arbiter::spawn(
                addr.send(StopServer { graceful: true })
                    .then(move |_| manager.send(StartServer { config, attempt: 0 }))
                    .map_err(|e| error!("Error while restarting API server: {}", e)),
            );
        }
    }
}

impl Actor for ApiManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.start_api_servers();
        let endpoints_rx = self
            .endpoints_rx
            .take()
            .expect("`Actor::started()` called twice for `ApiManager`");
        Self::add_stream(endpoints_rx, ctx);
    }
}

#[derive(Debug)]
struct StartServer {
    config: ApiRuntimeConfig,
    attempt: u16,
}

impl Message for StartServer {
    type Result = ();
}

impl Handler<StartServer> for ApiManager {
    type Result = ();

    fn handle(&mut self, mut msg: StartServer, ctx: &mut Context<Self>) -> Self::Result {
        info!(
            "Handling server start: {:?} (attempt #{})",
            msg.config,
            msg.attempt + 1
        );
        let addr = match self.start_server(msg.config.clone()) {
            Ok(addr) => addr,
            Err(e) => {
                warn!("Error handling service start {:?}: {}", msg.config, e);
                if msg.attempt == self.runtime_config.server_restart_max_retries {
                    error!("Cannot spawn server with config {:?}", msg.config);
                    ctx.terminate();
                } else {
                    msg.attempt += 1;
                    ctx.notify_later(
                        msg,
                        Duration::from_millis(self.runtime_config.server_restart_retry_timeout),
                    );
                }
                return;
            }
        };
        self.api_runtime_addresses.insert(addr, msg.config);
    }
}

/// Updates user-provided endpoints, restarting all HTTP service managed by the addressed
/// `ApiManager`.
#[derive(Debug, Clone)]
pub struct UpdateEndpoints {
    /// Complete list of endpoints.
    pub user_endpoints: Vec<(String, ApiBuilder)>,
}

impl Message for UpdateEndpoints {
    type Result = ();
}

impl StreamHandler<UpdateEndpoints, ()> for ApiManager {
    fn handle(&mut self, msg: UpdateEndpoints, ctx: &mut Context<Self>) {
        info!("Server restart requested");
        self.user_endpoints = msg.user_endpoints;
        self.initiate_restart(ctx.address());
    }
}
