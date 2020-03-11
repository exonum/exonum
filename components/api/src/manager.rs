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

use actix_cors::{Cors, CorsFactory};
use actix_rt::time::delay_for;
use actix_web::{dev::Server, web, App, HttpServer};
use futures::{
    future::{join_all, try_join_all},
    Stream, StreamExt, TryFutureExt,
};

use std::{collections::HashMap, io, net::SocketAddr, time::Duration};

use crate::{AllowOrigin, ApiAccess, ApiAggregator, ApiBuilder};

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

    fn cors_factory(&self) -> CorsFactory {
        self.allow_origin
            .clone()
            .map_or_else(Cors::default, CorsFactory::from)
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

/// Updates variable endpoints of the service, restarting all HTTP servers managed by the addressed
/// `ApiManager`. The endpoints initially supplied to the `ApiManager` during its construction
/// are not affected.
#[derive(Debug, Clone)]
pub struct UpdateEndpoints {
    /// Complete list of endpoints.
    pub endpoints: Vec<(String, ApiBuilder)>,
}

async fn with_retries<T>(
    mut action: impl FnMut() -> io::Result<T>,
    attempts: u16,
    timeout: u64,
) -> io::Result<T> {
    let timeout = Duration::from_millis(timeout);

    for _ in 1..=attempts {
        match action() {
            Ok(value) => return Ok(value),
            Err(e) => {
                log::warn!("Action failed: {}", e);
                delay_for(timeout).await;
            }
        }
    }

    Err(io::Error::new(
        io::ErrorKind::Other,
        "Cannot complete action",
    ))
}

/// Actor responsible for API management. The actor encapsulates endpoint handlers and
/// is capable of updating them via `UpdateEndpoints`.
#[derive(Debug)]
pub struct ApiManager {
    config: ApiManagerConfig,
    servers: Vec<Server>,
    endpoints: Vec<(String, ApiBuilder)>,
}

impl ApiManager {
    /// Creates a new API manager instance with the specified runtime configuration.
    pub fn new(config: ApiManagerConfig) -> Self {
        Self {
            config,
            servers: Vec::new(),
            endpoints: Vec::new(),
        }
    }

    async fn start_servers(&mut self) -> io::Result<()> {
        log::trace!("Servers start requested.");

        let start_servers = self.config.servers.iter().map(|(&access, server_config)| {
            let mut aggregator = self.config.api_aggregator.clone();
            aggregator.extend(self.endpoints.clone());
            let server_config = server_config.to_owned();

            with_retries(
                move || Self::start_server(aggregator.clone(), access, server_config.clone()),
                self.config.server_restart_max_retries,
                self.config.server_restart_retry_timeout,
            )
        });
        let servers = try_join_all(start_servers).await?;

        for (server, (&access, server_config)) in servers.iter().zip(&self.config.servers) {
            let listen_addr = server_config.listen_address;
            actix_rt::spawn(server.clone().unwrap_or_else(move |e| {
                log::error!("{} server on {} failed: {}", access, listen_addr, e);
                // TODO: should the server be restarted on error?
            }));
        }

        self.servers = servers;
        Ok(())
    }

    async fn stop_servers(&mut self) {
        log::trace!("Servers stop requested.");

        join_all(self.servers.drain(..).map(|server| server.stop(false))).await;
    }

    /// Starts API manager actor with the specified endpoints update stream.
    pub async fn run<S>(mut self, mut endpoints_rx: S) -> io::Result<()>
    where
        S: Stream<Item = UpdateEndpoints> + Unpin,
    {
        while let Some(request) = endpoints_rx.next().await {
            log::info!("Server restart requested");
            self.stop_servers().await;
            self.endpoints = request.endpoints;
            self.start_servers().await?;
        }

        Ok(())
    }

    fn start_server(
        aggregator: ApiAggregator,
        access: ApiAccess,
        server_config: WebServerConfig,
    ) -> io::Result<Server> {
        let listen_address = server_config.listen_address;
        log::info!("Starting {} web api on {}", access, listen_address);

        let server = HttpServer::new(move || {
            App::new()
                .wrap(server_config.cors_factory())
                .service(aggregator.extend_backend(access, web::scope("api")))
        })
        .disable_signals()
        .bind(listen_address)?
        .run();
        Ok(server)
    }
}
