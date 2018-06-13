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

use blockchain::Blockchain;
use crypto::{PublicKey, SecretKey};
use node::ApiSender;

/// Provides the current blockchain state to API handlers.
#[derive(Debug, Clone)]
pub struct ServiceApiState {
    blockchain: Blockchain,
}

impl ServiceApiState {
    /// Constructs state for the given blockchain.
    pub fn new(blockchain: Blockchain) -> ServiceApiState {
        ServiceApiState { blockchain }
    }

    /// Returns a reference to the blockchain of this node.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Returns the public key of the current node.
    pub fn public_key(&self) -> &PublicKey {
        &self.blockchain.service_keypair.0
    }

    /// Returns the secret key of the current node.
    pub fn secret_key(&self) -> &SecretKey {
        &self.blockchain.service_keypair.1
    }

    /// Returns a reference to the api sender.
    pub fn sender(&self) -> &ApiSender {
        &self.blockchain.api_sender
    }
}
