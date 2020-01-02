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

#![warn(missing_debug_implementations, unsafe_code, bare_trait_objects)]

//! A collection of tests of the Exonum node.

pub mod blockchain;
pub mod proto;

use exonum::node::{Node, ShutdownHandle};
use futures::Future;

use std::thread::{self, JoinHandle};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct RunHandle {
    node_thread: JoinHandle<()>,
    shutdown_handle: ShutdownHandle,
}

impl RunHandle {
    pub fn new(node: Node) -> Self {
        let shutdown_handle = node.shutdown_handle();
        Self {
            shutdown_handle,
            node_thread: thread::spawn(|| node.run().unwrap()),
        }
    }

    pub fn join(self) {
        self.shutdown_handle.shutdown().wait().unwrap();
        self.node_thread.join().unwrap();
    }
}
