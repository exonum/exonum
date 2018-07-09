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

use failure;

use std::sync::{mpsc};

use blockchain::Blockchain;
use node::ApiSender;

/// Type definition for additional workers.
pub type ServiceWorker =
    dyn Fn(ServiceWorkerContext) -> Result<(), failure::Error> + 'static + Send + Sync;

/// TODO
#[derive(Debug)]
pub struct ServiceWorkerContext {
    blockchain: Blockchain,
    cancellation_handler: mpsc::Receiver<()>,
}

impl ServiceWorkerContext {
    pub(crate) fn new(blockchain: Blockchain) -> (mpsc::Sender<()>, ServiceWorkerContext) {
        let (tx, rx) = mpsc::channel();
        (
            tx,
            ServiceWorkerContext {
                blockchain,
                cancellation_handler: rx,
            },
        )
    }

    /// Returns a reference to the blockchain of this node.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Returns a reference to the API sender.
    pub fn sender(&self) -> &ApiSender {
        &self.blockchain.api_sender
    }

    /// Returns true if node is already running.
    pub fn is_running(&self) -> bool {
        match self.cancellation_handler.try_recv() {
            Ok(_) => false,
            Err(mpsc::TryRecvError::Disconnected) => false,
            Err(mpsc::TryRecvError::Empty) => true,
        }
    }
}
