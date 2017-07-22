// Copyright 2017 The Exonum Team
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

extern crate rand;

use rand::Rng;

use std::net::SocketAddr;
use std::error::Error;

use messages::{Any, RawMessage, Connect, Status, Message, RequestPeers};
use helpers::Height;
use events::Channel;
use super::{NodeHandler, RequestData, ExternalMessage, NodeTimeout};

impl<S> NodeHandler<S>
where
    S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>,
{
    /// Redirects message to the corresponding `handle_...` function.
    pub fn handle_message(&mut self, raw: RawMessage) {
        // TODO: check message headers (network id, protocol version)
        // FIXME: call message.verify method
        //     if !raw.verify() {
        //         return;
        //     }

        match Any::from_raw(raw) {
            Ok(Any::Connect(msg)) => self.handle_connect(msg),
            Ok(Any::Status(msg)) => self.handle_status(msg),
            Ok(Any::Consensus(msg)) => self.handle_consensus(msg),
            Ok(Any::Request(msg)) => self.handle_request(msg),
            Ok(Any::Block(msg)) => self.handle_block(msg),
            Ok(Any::Transaction(msg)) => self.handle_tx(msg),
            Err(err) => {
                error!("Invalid message received: {:?}", err.description());
            }
        }
    }

    /// Handles the `Connected` event. Node's `Connect` message is sent as response.
    pub fn handle_connected(&mut self, addr: &SocketAddr) {
        info!("Connected to: {}", addr);
        let message = self.state.our_connect_message().clone();
        self.send_to_addr(addr, message.raw());
    }

    /// Handles the `Disconnected` event. Node will try to connect to that address again if it was
    /// in the validators list.
    pub fn handle_disconnected(&mut self, addr: &SocketAddr) {
        info!("Disconnected from: {}", addr);
        let need_reconnect = self.state.remove_peer_with_addr(addr);
        if need_reconnect {
            self.connect(addr);
        }
    }

    /// Handles the `Connect` message and connects to a peer as result.
    pub fn handle_connect(&mut self, message: Connect) {
        // TODO add spam protection
        let address = message.addr();
        if address == self.state.our_connect_message().addr() {
            trace!("Received Connect with same address as our external_address.");
            return;
        }

        if !self.state.whitelist().allow(message.pub_key()) {
            error!(
                "Received connect message from {:?} peer which not in whitelist.",
                message.pub_key()
            );
            return;
        }

        let public_key = *message.pub_key();
        if !message.verify_signature(&public_key) {
            error!(
                "Received connect-message with incorrect signature, msg={:?}",
                message
            );
            return;
        }

        // Check if we have another connect message from peer with the given public_key.
        let mut need_connect = true;
        if let Some(saved_message) = self.state.peers().get(&public_key) {
            if saved_message.time() > message.time() {
                error!("Received outdated Connect message from {}", address);
                return;
            } else if saved_message.time() < message.time() {
                need_connect = saved_message.addr() != message.addr();
            } else if saved_message.addr() != message.addr() {
                error!("Received weird Connect message from {}", address);
                return;
            }
        }
        info!(
            "Received Connect message from {}, {}",
            address,
            need_connect
        );
        self.state.add_peer(public_key, message);
        if need_connect {
            // TODO: reduce double sending of connect message
            info!("Send Connect message to {}", address);
            self.connect(&address);
        }
    }

    /// Handles the `Status` message. Node sends `RequestBlock` as response if height in the
    /// message is higher than node's height.
    pub fn handle_status(&mut self, msg: Status) {
        let height = self.state.height();
        trace!(
            "HANDLE STATUS: current height = {}, msg height = {}",
            height,
            msg.height()
        );

        if !self.state.whitelist().allow(msg.from()) {
            error!(
                "Received status message from peer = {:?} which not in whitelist.",
                msg.from()
            );
            return;
        }

        // Handle message from future height
        if msg.height() > height {
            let peer = msg.from();

            if !msg.verify_signature(peer) {
                error!(
                    "Received status message with incorrect signature, msg={:?}",
                    msg
                );
                return;
            }

            // Check validator height info
            if msg.height() > self.state.node_height(peer) {
                // Update validator height
                self.state.set_node_height(*peer, msg.height());
            }

            // Request block
            self.request(RequestData::Block(height), *peer);
        }
    }

    /// Handles the `RequestPeers` message. Node sends `Connect` messages of other peers as result.
    pub fn handle_request_peers(&mut self, msg: RequestPeers) {
        let peers: Vec<Connect> = self.state.peers().iter().map(|(_, b)| b.clone()).collect();
        trace!(
            "HANDLE REQUEST PEERS: Sending {:?} peers to {:?}",
            peers,
            msg.from()
        );

        for peer in peers {
            self.send_to_peer(*msg.from(), peer.raw());
        }
    }

    /// Handles `NodeTimeout::Status`, broadcasts the `Status` message if it isn't outdated as
    /// result.
    pub fn handle_status_timeout(&mut self, height: Height) {
        if self.state.height() == height {
            self.broadcast_status();
            self.add_status_timeout();
        }
    }
    /// Handles `NodeTimeout::PeerExchange`. Node sends the `RequestPeers` to a random peer.
    pub fn handle_peer_exchange_timeout(&mut self) {
        if !self.state.peers().is_empty() {
            let to = self.state.peers().len();
            let gen_peer_id = || -> usize {
                let mut rng = rand::thread_rng();
                rng.gen_range(0, to)
            };

            let peer = self.state
                .peers()
                .iter()
                .map(|x| x.1.clone())
                .nth(gen_peer_id())
                .unwrap();
            let peer = peer.clone();
            let msg = RequestPeers::new(
                self.state.consensus_public_key(),
                peer.pub_key(),
                self.state.consensus_secret_key(),
            );
            trace!("Request peers from peer with addr {:?}", peer.addr());
            self.send_to_peer(*peer.pub_key(), msg.raw());
        }
        self.add_peer_exchange_timeout();
    }
    /// Handles `NodeTimeout::UpdateApiState`.
    /// Node update internal `ApiState`.
    pub fn handle_update_api_state_timeout(&mut self) {
        self.api_state.update_node_state(&self.state);
        self.add_update_api_state_timeout();
    }

    /// Broadcasts the `Status` message to all peers.
    pub fn broadcast_status(&mut self) {
        let hash = self.blockchain.last_hash();
        let status = Status::new(
            self.state.consensus_public_key(),
            self.state.height(),
            &hash,
            self.state.consensus_secret_key(),
        );
        trace!("Broadcast status: {:?}", status);
        self.broadcast(status.raw());
    }
}
