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

use crate::api::backends::actix::{create_app, ApiRuntimeConfig};
use actix::prelude::*;
use actix_net::server::Server;
use actix_web::server::{HttpServer, StopServer};
use std::{fmt, io};

use crate::api::backends::actix::SystemRuntimeConfig;
use crate::api::ApiAggregator;
use futures::Future;
use std::collections::HashMap;

/// Actor responsible for API management.
pub struct ApiManager {
    runtime_config: SystemRuntimeConfig,
    api_runtime_addresses: HashMap<Addr<Server>, ApiRuntimeConfig>,
}

impl fmt::Debug for ApiManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ApiManager")
            .field("runtime_config", &self.runtime_config)
            .finish()
    }
}

/// Restart server actix message.
#[derive(Debug)]
pub struct RestartServer;

#[derive(Debug)]
struct StartServer {
    config: ApiRuntimeConfig,
}

impl ApiManager {
    pub fn new(runtime_config: SystemRuntimeConfig) -> Self {
        Self {
            runtime_config,
            api_runtime_addresses: HashMap::new(),
        }
    }

    fn start_api_servers(&mut self) {
        let aggregator = self.runtime_config.api_aggregator.clone();
        self.api_runtime_addresses = self
            .runtime_config
            .api_runtimes
            .iter()
            .cloned()
            .map(|runtime_config| {
                let server_address = Self::start_server(runtime_config.clone(), aggregator.clone())
                    .expect("Failed to start API server");
                (server_address, runtime_config)
            })
            .collect();
    }

    fn start_server(
        runtime_config: ApiRuntimeConfig,
        aggregator: ApiAggregator,
    ) -> io::Result<Addr<Server>> {
        let access = runtime_config.access;
        let listen_address = runtime_config.listen_address;
        info!("Starting {} web api on {}", access, listen_address);
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
                    .then(move |_| manager.send(StartServer { config }))
                    .map_err(|e| error!("Error while restarting API server: {}", e)),
            );
        }
    }
}

impl Actor for ApiManager {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.start_api_servers();
    }
}

impl Message for RestartServer {
    type Result = ();
}

impl Handler<RestartServer> for ApiManager {
    type Result = ();

    fn handle(&mut self, _msg: RestartServer, ctx: &mut Context<Self>) -> Self::Result {
        info!("Server restart requested");
        self.initiate_restart(ctx.address());
    }
}

impl Message for StartServer {
    type Result = ();
}
impl Handler<StartServer> for ApiManager {
    type Result = ();

    fn handle(&mut self, msg: StartServer, _ctx: &mut Context<Self>) -> Self::Result {
        info!("Handling start server");
        let aggregator = self.runtime_config.api_aggregator.clone();
        let addr = Self::start_server(msg.config.clone(), aggregator).unwrap();
        self.api_runtime_addresses.insert(addr, msg.config);
    }
}
