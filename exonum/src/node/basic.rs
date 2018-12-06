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

use rand::{self, Rng};

use super::{NodeHandler, NodeRole, RequestData};
use crypto::PublicKey;
use events::error::LogError;
use events::network::ConnectedPeerAddr;
use helpers::Height;
use messages::{Connect, Message, PeersRequest, Responses, Service, Signed, Status};

impl NodeHandler {
    /// Redirects message to the corresponding `handle_...` function.
    pub fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Consensus(msg) => self.handle_consensus(msg),
            Message::Requests(ref msg) => self.handle_request(msg),

            Message::Service(Service::Connect(msg)) => self.handle_connect(msg),
            Message::Service(Service::Status(msg)) => self.handle_status(&msg),
            // ignore tx duplication error,
            Message::Service(Service::RawTransaction(msg)) => drop(self.handle_tx(msg)),
            Message::Responses(Responses::BlockResponse(msg)) => {
                self.handle_block(&msg).log_error()
            }
            Message::Responses(Responses::TransactionsResponse(msg)) => {
                self.handle_txs_batch(&msg).log_error()
            }
        }
    }

    /// Handles the `Connected` event. Node's `Connect` message is sent as response
    /// if received `Connect` message is correct.
    pub fn handle_connected(&mut self, address: &ConnectedPeerAddr, connect: Signed<Connect>) {
        info!("Received Connect message from peer: {:?}", address);
        // TODO: use `ConnectInfo` instead of connect-messages. (ECR-1452)
        self.state.add_connection(connect.author(), address.clone());
        self.handle_connect(connect);
    }

    /// Handles the `Disconnected` event. Node will try to connect to that address again if it was
    /// in the validators list.
    pub fn handle_disconnected(&mut self, key: PublicKey) {
        info!("Disconnected from: {}", key);
        self.remove_peer_with_addr(key);
    }

    /// Handles the `UnableConnectToPeer` event. Node will try to connect to that address again
    /// if it was in the validators list.
    pub fn handle_unable_to_connect(&mut self, key: PublicKey) {
        info!("Could not connect to: {}", key);
        self.remove_peer_with_addr(key);
    }

    /// Removes peer from the state and from the cache. Node will try to connect to that address
    /// again if it was in the validators list.
    fn remove_peer_with_addr(&mut self, key: PublicKey) {
        self.state.remove_peer_with_pubkey(&key);
        self.blockchain.remove_peer_with_pubkey(&key);
        let is_validator = self.state.peer_is_validator(&key);
        let in_connect_list = self.state.peer_in_connect_list(&key);
        if is_validator && in_connect_list {
            self.connect(key);
        }
    }

    /// Handles the `Connect` message and connects to a peer as result.
    pub fn handle_connect(&mut self, message: Signed<Connect>) {
        // TODO Add spam protection (ECR-170)
        // TODO: drop connection if checks have failed. (ECR-1837)
        let address = message.pub_addr().to_owned();
        if address == self.state.our_connect_message().pub_addr() {
            trace!("Received Connect with same address as our external_address.");
            return;
        }

        let public_key = message.author();
        if public_key == self.state.our_connect_message().author() {
            trace!("Received Connect with same pub_key as ours.");
            return;
        }

        if !self.state.connect_list().is_peer_allowed(&public_key) {
            error!(
                "Received connect message from {:?} peer which not in ConnectList.",
                public_key
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
                need_connect = saved_message.pub_addr() != message.pub_addr();
            } else if saved_message.pub_addr() == message.pub_addr() {
                need_connect = false;
            } else {
                error!("Received weird Connect message from {}", address);
                return;
            }
            if saved_message.pub_addr() != message.pub_addr() {
                info!(
                    "Updating connect list for peer: {} with new addr: {}",
                    public_key,
                    message.pub_addr()
                );
                self.state
                    .connect_list()
                    .update_peer(&public_key, message.pub_addr().to_string())
            }
        }
        self.state.add_peer(public_key, message.clone());
        info!(
            "Received Connect message from {}. Need to connect: {}",
            address, need_connect,
        );
        self.blockchain.save_peer(&public_key, message);
        if need_connect {
            // TODO: reduce double sending of connect message
            info!("Send Connect message to {}", address);
            //TODO: remove responding connect [ECR-2385]
            self.connect(public_key);
        }
    }

    /// Handles the `Status` message. Node sends `BlockRequest` as response if height in the
    /// message is higher than node's height.
    pub fn handle_status(&mut self, msg: &Signed<Status>) {
        let height = self.state.height();
        trace!(
            "HANDLE STATUS: current height = {}, msg height = {}",
            height,
            msg.height()
        );

        if !self.state.connect_list().is_peer_allowed(&msg.author()) {
            error!(
                "Received status message from peer = {:?} which not in ConnectList.",
                msg.author()
            );
            return;
        }

        // Handle message from future height
        if msg.height() > height {
            let peer = msg.author();

            // Check validator height info
            if msg.height() > self.state.node_height(&peer) {
                // Update validator height
                self.state.set_node_height(peer, msg.height());
            }

            // Request block
            self.request(RequestData::Block(height), peer);
        }
    }

    /// Handles the `PeersRequest` message. Node sends `Connect` messages of other peers as result.
    pub fn handle_request_peers(&mut self, msg: &Signed<PeersRequest>) {
        let peers: Vec<Signed<Connect>> =
            self.state.peers().iter().map(|(_, b)| b.clone()).collect();
        trace!(
            "HANDLE REQUEST PEERS: Sending {:?} peers to {:?}",
            peers,
            msg.author()
        );

        for peer in peers {
            self.send_to_peer(msg.author(), peer);
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
    /// Handles `NodeTimeout::PeerExchange`. Node sends the `PeersRequest` to a random peer.
    pub fn handle_peer_exchange_timeout(&mut self) {
        if !self.state.peers().is_empty() {
            let to = self.state.peers().len();
            let gen_peer_id = || -> usize {
                let mut rng = rand::thread_rng();
                rng.gen_range(0, to)
            };

            let peer = self
                .state
                .peers()
                .iter()
                .map(|x| x.1.clone())
                .nth(gen_peer_id())
                .unwrap();
            let peer = peer.clone();
            let msg = PeersRequest::new(&peer.author());
            trace!("Request peers from peer with addr {:?}", peer.pub_addr());
            let message = self.sign_message(msg);
            self.send_to_peer(peer.author(), message);
        }
        self.add_peer_exchange_timeout();
    }
    /// Handles `NodeTimeout::UpdateApiState`.
    /// Node update internal `ApiState` and `NodeRole`.
    pub fn handle_update_api_state_timeout(&mut self) {
        self.api_state.update_node_state(&self.state);
        self.node_role = NodeRole::new(self.state.validator_id());
        self.add_update_api_state_timeout();
    }

    /// Broadcasts the `Status` message to all peers.
    pub fn broadcast_status(&mut self) {
        let hash = self.blockchain.last_hash();
        let status = Status::new(self.state.height(), &hash);
        trace!("Broadcast status: {:?}", status);

        let message = self.sign_message(status);
        self.broadcast(message);
    }
}
