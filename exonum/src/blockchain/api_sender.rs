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

// TODO: try using separate trait instead of defining `ExternalMessage` here.

use exonum_crypto::PublicKey;
use failure::Fail;
use futures::{sync::mpsc, Future, Sink};

use std::fmt;

use crate::messages::{AnyTx, Verified};

/// External messages sent to the node via `ApiSender`.
///
/// # Stability
///
/// This type and its methods are considered an implementation detail of the Exonum node and are
/// thus exempt from semantic versioning.
#[doc(hidden)]
#[derive(Debug)]
pub enum ExternalMessage {
    /// Add a new connection.
    PeerAdd {
        /// Peer address.
        address: String,
        /// Peer public key.
        public_key: PublicKey,
    },
    /// Transaction that implements the `Transaction` trait.
    Transaction(Verified<AnyTx>),
    /// Enable or disable the node.
    Enable(bool),
    /// Shutdown the node.
    Shutdown,
}

/// Transactions sender.
#[derive(Clone)]
pub struct ApiSender(mpsc::Sender<ExternalMessage>);

impl ApiSender {
    /// Creates new `ApiSender` with given channel.
    #[doc(hidden)]
    pub fn new(inner: mpsc::Sender<ExternalMessage>) -> Self {
        ApiSender(inner)
    }

    /// Creates a dummy sender which is not connected to the node and thus cannot send messages.
    pub fn closed() -> Self {
        ApiSender(mpsc::channel(0).0)
    }

    /// Sends an arbitrary `ExternalMessage` to the node.
    ///
    /// # Return value
    ///
    /// The failure means that the node is being shut down.
    ///
    /// # Stability
    ///
    /// This method is considered unstable because its misuse can lead to node breakage.
    #[doc(hidden)]
    pub fn send_external_message(
        &self,
        message: ExternalMessage,
    ) -> impl Future<Item = (), Error = SendError> {
        self.0
            .clone()
            .send(message)
            .map(drop)
            .map_err(|_| SendError(()))
    }

    /// Broadcasts transaction to other nodes in the blockchain network. This is an asynchronous
    /// operation that can take some time if the node is overloaded with requests.
    ///
    /// # Return value
    ///
    /// The failure means that the node is being shut down.
    pub fn broadcast_transaction(
        &self,
        tx: Verified<AnyTx>,
    ) -> impl Future<Item = (), Error = SendError> {
        self.send_external_message(ExternalMessage::Transaction(tx))
    }
}

impl fmt::Debug for ApiSender {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("ApiSender").field(&"..").finish()
    }
}

/// Errors that can occur during sending a message to the node via `ApiSender` or `ShutdownHandle`.
#[derive(Debug, Fail)]
#[fail(display = "Failed to send API request to the node: the node is being shut down")]
pub struct SendError(());
