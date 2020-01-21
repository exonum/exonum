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

use failure::Fail;
use futures::{sync::mpsc, Future, Sink};

use std::fmt;

use crate::messages::{AnyTx, Verified};

/// Asynchronous sender of messages (transactions by default). The receiver of messages is
/// usually an Exonum node, which then processes them with the consensus algorithm.
pub struct ApiSender<T = Verified<AnyTx>>(mpsc::Sender<T>);

impl<T> Clone for ApiSender<T> {
    fn clone(&self) -> Self {
        ApiSender(self.0.clone())
    }
}

impl<T: Send + 'static> ApiSender<T> {
    /// Creates new `ApiSender` with the given channel.
    pub fn new(inner: mpsc::Sender<T>) -> Self {
        ApiSender(inner)
    }

    /// Creates a dummy sender which is not connected to anything and thus cannot send messages.
    pub fn closed() -> Self {
        ApiSender(mpsc::channel(0).0)
    }

    /// Sends a message to the node.
    ///
    /// # Return value
    ///
    /// The failure means that the node is being shut down.
    pub fn send_message(&self, message: T) -> impl Future<Item = (), Error = SendError> {
        self.0
            .clone()
            .send(message)
            .map(drop)
            .map_err(|_| SendError(()))
    }
}

impl ApiSender {
    /// Sends a transaction over the channel. If this sender is connected to a node,
    /// this will broadcast the transaction to all nodes in the blockchain network.
    ///
    /// This is an asynchronous operation that can take some time if the node is overloaded
    /// with requests.
    ///
    /// # Return value
    ///
    /// The failure means that the node is being shut down.
    pub fn broadcast_transaction(
        &self,
        tx: Verified<AnyTx>,
    ) -> impl Future<Item = (), Error = SendError> {
        self.send_message(tx)
    }
}

impl<T> fmt::Debug for ApiSender<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("ApiSender").field(&"..").finish()
    }
}

/// Errors that can occur during sending a message to the node via `ApiSender`.
#[derive(Debug, Fail)]
#[fail(display = "Failed to send API request to the node: the node is being shut down")]
pub struct SendError(());
