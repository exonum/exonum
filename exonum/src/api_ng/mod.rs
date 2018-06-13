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
pub use self::state::{ServiceApiState, ServiceApiStateMut};
pub use self::with::{FutureResult, NamedWith, Result, With};

use serde::de::DeserializeOwned;
use serde::Serialize;

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
    fn endpoint<S, Q, I, R, F, E>(&mut self, name: &'static str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r S, Q) -> R + 'static + Clone,
        E: Into<With<S, Q, I, R, F>>,
        Self::Handler: From<NamedWith<S, Q, I, R, F>>,
    {
        let named_with = NamedWith::new(name, endpoint);
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
    pub fn endpoint<S, Q, I, R, F, E>(&mut self, name: &'static str, endpoint: E) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: for<'r> Fn(&'r S, Q) -> R + 'static + Clone,
        E: Into<With<S, Q, I, R, F>>,
        actix::RequestHandler: From<NamedWith<S, Q, I, R, F>>,
    {
        self.actix_backend.endpoint(name, endpoint);
        self
    }

    /// Returns reference to the underlying web backend
    pub fn web_backend(&mut self) -> &mut actix::ApiBuilder {
        &mut self.actix_backend
    }
}

/// Service API builder.
#[derive(Debug, Default)]
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

pub(crate) trait IntoApiBackend {
    fn extend<'a, I>(self, items: I) -> Self
    where
        I: IntoIterator<Item = &'a (String, ServiceApiScope)>;
}

#[derive(Debug, Clone)]
pub(crate) struct ApiAggregator {
    public_scope: Vec<(String, ServiceApiScope)>,
    private_scope: Vec<(String, ServiceApiScope)>,
}

impl ApiAggregator {
    pub fn new(blockchain: Blockchain, shared_api_state: SharedNodeState) -> ApiAggregator {
        let state = ServiceApiStateMut::new(blockchain);

        let mut public_scope = Vec::new();
        let mut private_scope = Vec::new();
        // Adds public built-in APIs.
        public_scope.push(Self::public_explorer_api(&state));
        public_scope.push(Self::public_system_api(shared_api_state.clone()));
        // Adds private built-in APIs.
        private_scope.push(Self::private_system_api(
            state.blockchain(),
            shared_api_state,
        ));
        // Adds services APIs.
        state
            .blockchain()
            .service_map()
            .iter()
            .map(|(_, s)| s)
            .for_each(|service| {
                let mut builder = ServiceApiBuilder::new();
                service.wire_api(&mut builder);

                public_scope.push((service.service_name().to_owned(), builder.public_scope));
                private_scope.push((service.service_name().to_owned(), builder.private_scope));
            });

        ApiAggregator {
            public_scope,
            private_scope,
        }
    }

    /// TODO
    pub fn extend_public_api<B>(&self, backend: B) -> B
    where
        B: IntoApiBackend,
    {
        backend.extend(&self.public_scope)
    }

    /// TODO
    pub fn extend_private_api<B>(&self, backend: B) -> B
    where
        B: IntoApiBackend,
    {
        backend.extend(&self.private_scope)
    }

    fn public_system_api(shared_api_state: SharedNodeState) -> (String, ServiceApiScope) {
        let mut scope = ServiceApiScope::new();
        let system_api = self::node::public::SystemApi::new(shared_api_state);
        system_api.wire(&mut scope);
        ("system".to_owned(), scope)
    }

    fn public_explorer_api(state: &ServiceApiState) -> (String, ServiceApiScope) {
        let mut scope = ServiceApiScope::new();
        self::node::public::ExplorerApi::wire(state, &mut scope);
        ("explorer".to_owned(), scope)
    }

    fn private_system_api(
        blockchain: &Blockchain,
        shared_api_state: SharedNodeState,
    ) -> (String, ServiceApiScope) {
        let mut scope = ServiceApiScope::new();
        let node_info =
            self::node::private::NodeInfo::new(blockchain.service_map().iter().map(|(_, s)| s));
        let system_api = self::node::private::SystemApi::new(node_info, shared_api_state);
        system_api.wire(&mut scope);
        ("system".to_owned(), scope)
    }
}
