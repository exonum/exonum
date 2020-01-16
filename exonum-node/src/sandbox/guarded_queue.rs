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

use exonum::crypto::PublicKey;

use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
};

use crate::messages::Message;

/// Guarded queue for messages sent by the sandbox. If the queue is not empty when dropped,
/// it panics.
#[derive(Debug, Default)]
pub struct GuardedQueue(VecDeque<(PublicKey, Message)>);

impl Deref for GuardedQueue {
    type Target = VecDeque<(PublicKey, Message)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GuardedQueue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for GuardedQueue {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            if let Some((addr, msg)) = self.0.pop_front() {
                panic!("Sent unexpected message {:?} to {}", msg, addr);
            }
        }
    }
}
