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

//! Module responsible for actix web API management after new service is deployed.

use actix::prelude::*;
use actix_net::server::Server;
use actix_web::server::{HttpServer, StopServer};
use futures::{sync::mpsc, Future};

use std::{collections::HashMap, fmt, io, net::SocketAddr, time::Duration};

use crate::{backends::actix::create_app, AllowOrigin, ApiAccess, ApiAggregator, ApiBuilder};

/// Configuration parameters for a single web server.
#[derive(Debug, Clone)]
pub struct WebServerConfig {
    /// The socket address to bind.
    pub listen_address: SocketAddr,
    /// Optional CORS settings.
    pub allow_origin: Option<AllowOrigin>,
}

impl WebServerConfig {
    /// Creates a web server configuration for the given listen address.
    pub fn new(listen_address: SocketAddr) -> Self {
        Self {
            listen_address,
            allow_origin: None,
        }
    }
}

/// Configuration parameters for `ApiManager`.
#[derive(Debug, Clone)]
pub struct ApiManagerConfig {
    /// Active API runtimes.
    pub servers: HashMap<ApiAccess, WebServerConfig>,
    /// API aggregator with initial endpoint builders. The initial endpoints will not be
    /// affected by `UpdateEndpoints` messages.
    pub api_aggregator: ApiAggregator,
    /// The interval in milliseconds between attempts of restarting HTTP-server in case
    /// the server failed to restart
    pub server_restart_retry_timeout: u64,
    /// The attempts counts of restarting HTTP-server in case the server failed to restart
    pub server_restart_max_retries: u16,
}

/// Actor responsible for API management. The actor encapsulates endpoint handlers and
/// is capable of updating them via `UpdateEndpoints`.
pub struct ApiManager {
    config: ApiManagerConfig,
    server_addresses: HashMap<ApiAccess, Addr<Server>>,
    variable_endpoints: Vec<(String, ApiBuilder)>,
    endpoints_rx: Option<mpsc::Receiver<UpdateEndpoints>>,
}

impl fmt::Debug for ApiManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiManager")
            .field("config", &self.config)
            .finish()
    }
}

impl ApiManager {
    /// Creates a new API manager instance with the specified runtime configuration and
    /// the receiver of the `UpdateEndpoints` events.
    pub fn new(config: ApiManagerConfig, endpoints_rx: mpsc::Receiver<UpdateEndpoints>) -> Self {
        Self {
            config,
            server_addresses: HashMap::new(),
            variable_endpoints: vec![],
            endpoints_rx: Some(endpoints_rx),
        }
    }

    fn start_api_servers(&mut self) {
        self.server_addresses = self
            .config
            .servers
            .iter()
            .map(|(&access, server_config)| {
                let server_address = self
                    .start_server(access, server_config.to_owned())
                    .expect("Failed to start API server");
                (access, server_address)
            })
            .collect();
    }

    fn start_server(
        &self,
        access: ApiAccess,
        server_config: WebServerConfig,
    ) -> io::Result<Addr<Server>> {
        let listen_address = server_config.listen_address;
        log::info!("Starting {} web api on {}", access, listen_address);

        let mut aggregator = self.config.api_aggregator.clone();
        aggregator.extend(self.variable_endpoints.clone());
        HttpServer::new(move || create_app(&aggregator, access, &server_config))
            .disable_signals()
            .bind(listen_address)
            .map(HttpServer::start)
    }

    fn initiate_restart(&mut self, manager: Addr<Self>) {
        log::info!("Restarting servers.");
        for (access, addr) in self.server_addresses.drain() {
            let manager = manager.clone();
            Arbiter::spawn(
                addr.send(StopServer { graceful: true })
                    .then(move |_| manager.send(StartServer { access, attempt: 0 }))
                    .map_err(|e| log::error!("Error while restarting API server: {}", e)),
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
    access: ApiAccess,
    attempt: u16,
}

impl Message for StartServer {
    type Result = ();
}

impl Handler<StartServer> for ApiManager {
    type Result = ();

    fn handle(&mut self, mut msg: StartServer, ctx: &mut Context<Self>) -> Self::Result {
        log::info!(
            "Handling server start: {:?} (attempt #{})",
            msg.access,
            msg.attempt + 1
        );

        let server_config = self.config.servers[&msg.access].clone();
        let addr = match self.start_server(msg.access, server_config) {
            Ok(addr) => addr,
            Err(e) => {
                log::warn!("Error handling {} server start: {}", msg.access, e);
                if msg.attempt == self.config.server_restart_max_retries {
                    log::error!("Cannot spawn {} server", msg.access);
                    ctx.terminate();
                } else {
                    msg.attempt += 1;
                    ctx.notify_later(
                        msg,
                        Duration::from_millis(self.config.server_restart_retry_timeout),
                    );
                }
                return;
            }
        };
        self.server_addresses.insert(msg.access, addr);
    }
}

/// Updates variable endpoints of the service, restarting all HTTP servers managed by the addressed
/// `ApiManager`. The endpoints initially supplied to the `ApiManager` during its construction
/// are not affected.
#[derive(Debug, Clone)]
pub struct UpdateEndpoints {
    /// Complete list of endpoints.
    pub endpoints: Vec<(String, ApiBuilder)>,
}

impl Message for UpdateEndpoints {
    type Result = ();
}

impl StreamHandler<UpdateEndpoints, ()> for ApiManager {
    fn handle(&mut self, msg: UpdateEndpoints, ctx: &mut Context<Self>) {
        log::info!("Server restart requested");
        self.variable_endpoints = msg.endpoints;
        self.initiate_restart(ctx.address());
    }
}
