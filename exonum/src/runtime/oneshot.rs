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

//! A channel for sending a deployment status between threads.

use std::sync::mpsc;

use super::{ErrorKind, ExecutionError};

/// Create a new one-shot channel for sending deployment status,
/// returning the sender/receiver halves.
pub fn channel() -> (Sender, Receiver) {
    let (tx, rx) = mpsc::channel();
    (Sender(tx), Receiver(rx))
}

/// The receiving half of [`channel`] type. This half can only be owned by one thread.
///
/// [`channel`]: fn.channel.html
#[derive(Debug)]
pub struct Receiver(mpsc::Receiver<Result<(), ExecutionError>>);

impl Receiver {
    /// Attempts to wait for a value on this receiver, returning an error if the
    /// corresponding channel has hung up.
    pub(crate) fn wait(self) -> Result<(), ExecutionError> {
        self.0.recv().map_err(|e| {
            ExecutionError::new(
                ErrorKind::Unexpected,
                format!(
                    "An error during waiting for deployment status occurred: {:?}",
                    e
                ),
            )
        })?
    }
}

/// The sending-half of [`channel`] type. This half can only be
/// owned by one thread, but it can be cloned to send to other threads.
///
/// Deployment status can be sent through this channel with [`send`].
///
/// [`channel`]: fn.channel.html
/// [`send`]: struct.Sender.html#method.send
#[derive(Debug)]
pub struct Sender(mpsc::Sender<Result<(), ExecutionError>>);

impl Sender {
    /// Sends a deployment status on this channel.
    ///
    /// This method will never block the current thread.
    pub fn send(self, deployment_status: Result<(), ExecutionError>) {
        if let Err(e) = self.0.send(deployment_status) {
            log::warn!(
                "Unable to send deployment status: an error occurred {:?}",
                e
            );
        }
    }
}
