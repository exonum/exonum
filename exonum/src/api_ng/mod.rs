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

use serde::de::DeserializeOwned;
use serde::Serialize;

pub use self::state::{ServiceApiState, ServiceApiStateMut};
pub use self::with::{FutureResult, NamedWith, Result, With};

pub mod backends;
pub mod error;
mod state;
mod with;

/// TODO
pub trait ServiceApi {
    /// TODO
    fn wire(&self, _builder: &mut ServiceApiBuilder) {}
}

/// Trait defines object that could be used as an API backend.
pub trait ServiceApiBackend: Sized {
    /// Concrete endpoint handler in the backend.
    type Handler;

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
}

/// TODO
#[derive(Debug)]
pub struct ServiceApiScope;

/// TODO
#[derive(Debug)]
pub struct ServiceApiBuilder {
    public_scope: ServiceApiScope,
    private_scope: ServiceApiScope,
}
