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

//! API and corresponding utilities.

pub use self::error::Error;
pub use self::state::ServiceApiState;
pub use self::with::{FutureResult, Immutable, Mutable, NamedWith, Result, With};

use serde::de::DeserializeOwned;
use serde::Serialize;

use std::collections::BTreeMap;

use self::backends::actix;
use blockchain::{Blockchain, SharedNodeState};

pub mod backends;
pub mod error;
pub mod node;
mod state;
mod with;

/// Trait defines object that could be used as an API backend.
pub trait ServiceApiBackend: Sized {
    /// Concrete endpoint handler in the backend.
    type Handler;
    /// Concrete output API scope.
    type Scope;

    /// Adds the given endpoint handler to the backend.
    fn endpoint<N, Q, I, R, F, E>(&mut self, name: N, endpoint: E) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F, Immutable>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        let named_with = NamedWith {
            name: name.into(),
            inner: endpoint.into(),
        };
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Adds the given mutable endpoint handler to the backend.
    fn endpoint_mut<N, Q, I, R, F, E>(&mut self, name: N, endpoint: E) -> &mut Self
    where
        N: Into<String>,
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F, Mutable>>,
        Self::Handler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        let named_with = NamedWith {
            name: name.into(),
            inner: endpoint.into(),
        };
        self.raw_handler(Self::Handler::from(named_with))
    }

    /// Adds the raw endpoint handler for the given backend.
    fn raw_handler(&mut self, handler: Self::Handler) -> &mut Self;

    /// TODO
    fn wire(&self, output: Self::Scope) -> Self::Scope;
}

/// TODO
#[derive(Debug, Clone, Default)]
pub struct ServiceApiScope {
    pub(crate) actix_backend: actix::ApiBuilder,
}

impl ServiceApiScope {
    /// Creates a new service api scope.
    pub fn new() -> ServiceApiScope {
        ServiceApiScope::default()
    }

    /// Adds the given endpoint handler to the api scope.
    pub fn endpoint<Q, I, R, F, E>(&mut self, name: &'static str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F, Immutable>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Immutable>>,
    {
        self.actix_backend.endpoint(name, endpoint);
        self
    }

    /// Adds the given mutable endpoint handler to the api scope.
    pub fn endpoint_mut<Q, I, R, F, E>(&mut self, name: &'static str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r ServiceApiState, Q) -> R + 'static + Clone,
        E: Into<With<Q, I, R, F, Mutable>>,
        actix::RequestHandler: From<NamedWith<Q, I, R, F, Mutable>>,
    {
        self.actix_backend.endpoint_mut(name, endpoint);
        self
    }

    /// Returns reference to the underlying web backend.
    pub fn web_backend(&mut self) -> &mut actix::ApiBuilder {
        &mut self.actix_backend
    }
}

/// Service API builder.
#[derive(Debug, Clone, Default)]
pub struct ServiceApiBuilder {
    public_scope: ServiceApiScope,
    private_scope: ServiceApiScope,
}

impl ServiceApiBuilder {
    /// Creates a new service API builder.
    pub fn new() -> ServiceApiBuilder {
        ServiceApiBuilder::default()
    }

    /// Returns reference to the public api scope builder.
    pub fn public_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.public_scope
    }

    /// Returns reference to the private api scope builder.
    pub fn private_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.private_scope
    }
}

/// TODO
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiScope {
    /// TODO
    Public,
    /// TODO
    Private,
}

impl ::std::fmt::Display for ApiScope {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            ApiScope::Public => f.write_str("public"),
            ApiScope::Private => f.write_str("private"),
        }
    }
}

pub(crate) trait IntoApiBackend {
    fn extend<'a, I>(self, items: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a ServiceApiScope)>;
}

#[derive(Debug, Clone)]
pub(crate) struct ApiAggregator {
    pub(crate) blockchain: Blockchain,
    inner: BTreeMap<String, ServiceApiBuilder>,
}

impl ApiAggregator {
    pub fn new(blockchain: Blockchain, shared_api_state: SharedNodeState) -> ApiAggregator {
        let mut inner = BTreeMap::new();
        // Adds built-in APIs.
        inner.insert(
            "system".to_owned(),
            Self::system_api(&blockchain, shared_api_state),
        );
        inner.insert("explorer".to_owned(), Self::explorer_api());
        // Adds services APIs.
        inner.extend(blockchain.service_map().iter().map(|(_, service)| {
            let mut builder = ServiceApiBuilder::new();
            service.wire_api(&mut builder);
            // TODO think about prefixes for non web backends.
            let prefix = format!("services/{}", service.service_name());
            (prefix, builder)
        }));

        ApiAggregator { inner, blockchain }
    }

    /// TODO
    pub fn extend_public_api<B>(&self, backend: B) -> B
    where
        B: IntoApiBackend,
    {
        backend.extend(
            self.inner
                .iter()
                .map(|(name, builder)| (name.as_ref(), &builder.public_scope)),
        )
    }

    /// TODO
    pub fn extend_private_api<B>(&self, backend: B) -> B
    where
        B: IntoApiBackend,
    {
        backend.extend(
            self.inner
                .iter()
                .map(|(name, builder)| (name.as_ref(), &builder.private_scope)),
        )
    }

    fn explorer_api() -> ServiceApiBuilder {
        let mut builder = ServiceApiBuilder::new();
        <ServiceApiState as self::node::public::ExplorerApi>::wire(builder.public_scope());
        builder
    }

    fn system_api(blockchain: &Blockchain, shared_api_state: SharedNodeState) -> ServiceApiBuilder {
        let mut builder = ServiceApiBuilder::new();
        let node_info = self::node::private::NodeInfo::new(
            blockchain.service_map().iter().map(|(_, service)| service),
        );
        self::node::private::SystemApi::new(node_info, shared_api_state.clone())
            .wire(builder.private_scope());
        self::node::public::SystemApi::new(shared_api_state).wire(builder.public_scope());
        builder
    }
}
