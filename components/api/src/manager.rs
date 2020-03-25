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

#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, FromRawSocket, RawSocket};
use std::{
    collections::HashMap,
    io,
    net::{SocketAddr, TcpListener},
    time::Duration,
};

use crate::{AllowOrigin, ApiAccess, ApiAggregator, ApiBuilder};

/// Configuration parameters for a single web server.
#[derive(Debug, Clone)]
#[non_exhaustive]
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
    /// the server failed to restart.
    pub server_restart_retry_timeout: u64,
    /// The attempts counts of restarting HTTP-server in case the server failed to restart.
    pub server_restart_max_retries: u16,
}

/// Updates variable endpoints of the service, restarting all HTTP servers managed by the addressed
/// `ApiManager`. The endpoints initially supplied to the `ApiManager` during its construction
/// are not affected.
#[derive(Debug, Clone)]
pub struct UpdateEndpoints {
    /// Complete list of endpoints.
    endpoints: Vec<(String, ApiBuilder)>,
}

impl UpdateEndpoints {
    /// Creates an update with the previous server endpoints completely replaced
    /// by the provided endpoints.
    pub fn new(endpoints: Vec<(String, ApiBuilder)>) -> Self {
        Self { endpoints }
    }

    /// Iterates over paths updated in this object.
    pub fn updated_paths(&self) -> impl Iterator<Item = &str> {
        self.endpoints.iter().map(|(path, _)| path.as_str())
    }

    /// Converts this update into constituent endpoints.
    #[doc(hidden)] // used by testkit; not stable yet
    pub fn into_endpoints(self) -> Vec<(String, ApiBuilder)> {
        self.endpoints
    }
}

async fn with_retries<T>(
    mut action: impl FnMut() -> io::Result<T>,
    description: String,
    attempts: u16,
    timeout: u64,
) -> io::Result<T> {
    let timeout = Duration::from_millis(timeout);

    for attempt in 1..=attempts {
        log::trace!("{} (attempt #{})", description, attempt);
        match action() {
            Ok(value) => return Ok(value),
            Err(e) => {
                log::warn!("{} (attempt #{}) failed: {}", description, attempt, e);
                delay_for(timeout).await;
            }
        }
    }

    let msg = format!(
        "Cannot complete {} after {} attempts",
        description, attempts
    );
    Err(io::Error::new(io::ErrorKind::Other, msg))
}

#[derive(Debug)]
struct ServerHandle {
    inner: Server,
    #[cfg(windows)]
    raw_socket: RawSocket,
}

impl ServerHandle {
    #[cfg(not(windows))]
    async fn stop(self) {
        self.inner.stop(false).await;
    }

    #[cfg(windows)]
    #[allow(unsafe_code)]
    async fn stop(self) {
        self.inner.stop(false).await;

        // SAFETY: The safety of `from_raw_socket` relies on the fact that the socket is not used
        // by `actix` afterwards and is not freed by it before. That is, it relies
        // on the incorrect behavior of external libraries (`actix` *should* free the socket
        // on its side). Not a good guarantee, but it works with current `actix-web` (2.0.0,
        // with actual dependencies as of 2020-03-24).
        //
        // See actix/actix-web#1249 for details on the issue.
        let _socket = unsafe { TcpListener::from_raw_socket(self.raw_socket) };
    }
}

/// Component responsible for API management. The `ApiManager` encapsulates endpoint handlers and
/// is capable of updating them via `UpdateEndpoints`.
#[derive(Debug)]
pub struct ApiManager {
    config: ApiManagerConfig,
    servers: Vec<ServerHandle>,
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
            let action_description = format!(
                "starting {} api on {}",
                access, server_config.listen_address
            );

            with_retries(
                move || Self::start_server(aggregator.clone(), access, server_config.clone()),
                action_description,
                self.config.server_restart_max_retries,
                self.config.server_restart_retry_timeout,
            )
        });
        let servers = try_join_all(start_servers).await?;

        for (server, (&access, server_config)) in servers.iter().zip(&self.config.servers) {
            let listen_addr = server_config.listen_address;
            actix_rt::spawn(server.inner.clone().unwrap_or_else(move |e| {
                log::error!("{} server on {} failed: {}", access, listen_addr, e);
                // TODO: should the server be restarted on error?
            }));
        }

        self.servers = servers;
        Ok(())
    }

    async fn stop_servers(&mut self) {
        log::trace!("Servers stop requested.");

        join_all(self.servers.drain(..).map(ServerHandle::stop)).await;
    }

    /// Starts API manager actor with the specified endpoints update stream.
    pub async fn run<S>(mut self, endpoints_rx: S) -> io::Result<()>
    where
        S: Stream<Item = UpdateEndpoints> + Unpin,
    {
        let res = self.run_inner(endpoints_rx).await;
        // Stop the HTTP server(s) in any case.
        self.stop_servers().await;
        log::info!("HTTP servers shut down");
        res
    }

    async fn run_inner<S>(&mut self, mut endpoints_rx: S) -> io::Result<()>
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
    ) -> io::Result<ServerHandle> {
        let listen_address = server_config.listen_address;
        log::info!("Starting {} web api on {}", access, listen_address);

        let listener = TcpListener::bind(listen_address)?;
        #[cfg(windows)]
        let raw_socket = listener.as_raw_socket();

        let server = HttpServer::new(move || {
            App::new()
                .wrap(server_config.cors_factory())
                .service(aggregator.extend_backend(access, web::scope("api")))
        })
        .disable_signals()
        .listen(listener)?
        .run();

        Ok(ServerHandle {
            inner: server,
            #[cfg(windows)]
            raw_socket,
        })
    }
}
