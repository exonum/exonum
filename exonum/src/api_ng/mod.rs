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

pub use self::with::{FutureResult, NamedWith, Result, With};

pub mod error;
mod with;

use std::fmt;
use std::ops::Deref;

use blockchain::Blockchain;
use crypto::{PublicKey, SecretKey};
use node::ApiSender;

/// TODO
pub trait ServiceApi {
    /// TODO
    fn wire(&self, _builder: &mut ServiceApiBuilder) {}
}

/// TODO
#[derive(Debug)]
pub struct ServiceApiScope;

/// TODO
#[derive(Debug)]
pub struct ServiceApiBuilder;

/// Provides the current blockchain state to API handlers.
#[derive(Clone)]
pub struct ServiceApiState {
    blockchain: Blockchain,
    service_keypair: (PublicKey, SecretKey),
}

impl ServiceApiState {
    /// Returns a reference to the blockchain of this node.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Returns the public key of the current node.
    pub fn public_key(&self) -> &PublicKey {
        &self.service_keypair.0
    }

    /// Returns the secret key of the current node.
    pub fn secret_key(&self) -> &SecretKey {
        &self.service_keypair.1
    }
}

/// Mutable version of the `ServiceApiState` that also provides
/// the current node state to API handlers.
#[derive(Debug, Clone)]
pub struct ServiceApiStateMut {
    inner: ServiceApiState,
    sender: ApiSender,
}

impl ServiceApiStateMut {
    /// Constructs state from given parts.
    pub fn new(
        blockchain: Blockchain,
        sender: ApiSender,
        public_key: PublicKey,
        secret_key: SecretKey,
    ) -> ServiceApiStateMut {
        ServiceApiStateMut {
            inner: ServiceApiState {
                blockchain,
                service_keypair: (public_key, secret_key),
            },
            sender,
        }
    }

    /// Returns a reference to the api sender.
    pub fn sender(&self) -> &ApiSender {
        &self.sender
    }
}

impl fmt::Debug for ServiceApiState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ServiceApiState")
            .field("blockchain", &self.blockchain)
            .field("service_public_key", &self.service_keypair.0)
            .finish()
    }
}

impl Deref for ServiceApiStateMut {
    type Target = ServiceApiState;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
