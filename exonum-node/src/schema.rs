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

use exonum::{
    crypto::PublicKey,
    helpers::Round,
    merkledb::{
        access::{Access, AccessExt, RawAccessMut},
        ListIndex, MapIndex,
    },
    messages::Verified,
};

use std::iter;

use crate::messages::{Connect, Message};

const CONSENSUS_MESSAGES_CACHE: &str = "core.consensus_messages_cache";
const CONSENSUS_ROUND: &str = "core.consensus_round";
const PEERS_CACHE: &str = "core.peers_cache";

/// Schema for an Exonum node.
#[derive(Debug)]
pub(crate) struct NodeSchema<T> {
    access: T,
}

impl<T: Access> NodeSchema<T> {
    pub fn new(access: T) -> Self {
        Self { access }
    }

    /// Returns peers that have to be recovered in case of process restart
    /// after abnormal termination.
    pub fn peers_cache(&self) -> MapIndex<T::Base, PublicKey, Verified<Connect>> {
        self.access.get_map(PEERS_CACHE)
    }

    /// Returns consensus messages that have to be recovered in case of process restart
    /// after abnormal termination.
    pub fn consensus_messages_cache(&self) -> ListIndex<T::Base, Message> {
        self.access.get_list(CONSENSUS_MESSAGES_CACHE)
    }

    /// Returns the saved value of the consensus round. Returns the first round
    /// if it has not been saved.
    pub fn consensus_round(&self) -> Round {
        self.access
            .get_entry(CONSENSUS_ROUND)
            .get()
            .unwrap_or_else(Round::first)
    }
}

impl<T: Access> NodeSchema<T>
where
    T::Base: RawAccessMut,
{
    /// Saves the given consensus round value into the storage.
    pub fn set_consensus_round(&mut self, round: Round) {
        self.access.get_entry(CONSENSUS_ROUND).set(round);
    }

    /// Saves a collection of `SignedMessage`s to the consensus messages cache.
    pub fn save_messages<I>(&mut self, round: Round, iter: I)
    where
        I: IntoIterator<Item = Message>,
    {
        self.consensus_messages_cache().extend(iter);
        self.set_consensus_round(round);
    }

    pub fn save_message<M: Into<Message>>(&mut self, round: Round, message: M) {
        self.save_messages(round, iter::once(message.into()));
    }

    /// Saves the `Connect` message from a peer to the cache.
    pub fn save_peer(&mut self, pubkey: &PublicKey, peer: Verified<Connect>) {
        self.peers_cache().put(pubkey, peer);
    }

    /// Removes from the cache the `Connect` message from a peer.
    pub fn remove_peer_with_pubkey(&mut self, key: &PublicKey) {
        self.peers_cache().remove(key);
    }
}
