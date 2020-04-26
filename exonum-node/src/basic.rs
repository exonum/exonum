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

use exonum::{crypto::PublicKey, helpers::Height, merkledb::ObjectHash, messages::Verified};
use log::{error, info, trace};
use rand::Rng;

use crate::{
    events::{error::LogError, network::ConnectedPeerAddr},
    messages::{Connect, Message, PeersRequest, Responses, Service, Status},
    schema::NodeSchema,
    state::{PeerState, RequestData},
    NodeHandler, NodeRole,
};

impl NodeHandler {
    /// Redirects message to the corresponding `handle_...` function.
    pub(crate) fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Consensus(msg) => self.handle_consensus(msg),
            Message::Requests(ref msg) => self.handle_request(msg),

            Message::Service(Service::Connect(msg)) => self.handle_connect(msg),
            Message::Service(Service::Status(msg)) => self.handle_status(&msg),
            Message::Service(Service::AnyTx(msg)) => {
                if let Err(e) = self.handle_tx(msg.clone()) {
                    log::warn!(
                        "Failed to process transaction {:?} (hash = {:?}): {}",
                        msg.payload(),
                        msg.object_hash(),
                        e
                    );
                }
            }

            Message::Responses(Responses::BlockResponse(msg)) => {
                self.handle_block(msg);
            }
            Message::Responses(Responses::TransactionsResponse(msg)) => {
                self.handle_txs_batch(&msg).log_error()
            }
        }
    }

    /// Handles the `Connected` event. Node's `Connect` message is sent as response
    /// if received `Connect` message is correct.
    pub(crate) fn handle_connected(
        &mut self,
        address: &ConnectedPeerAddr,
        connect: Verified<Connect>,
    ) {
        info!("Received Connect message from peer: {:?}", address);
        // TODO: use `ConnectInfo` instead of connect-messages. (ECR-1452)
        self.state.add_connection(connect.author(), address.clone());
        self.handle_connect(connect);
    }

    /// Handles the `Disconnected` event. Node will try to connect to that address again if it was
    /// in the validators list.
    pub(crate) fn handle_disconnected(&mut self, key: PublicKey) {
        info!("Disconnected from: {}", key);
        self.remove_peer_with_addr(key);
    }

    /// Handles the `UnableConnectToPeer` event. Node will try to connect to that address again
    /// if it was in the validators list.
    pub(crate) fn handle_unable_to_connect(&mut self, key: PublicKey) {
        info!("Could not connect to: {}", key);
        self.remove_peer_with_addr(key);
    }

    /// Removes peer from the state and from the cache. Node will try to connect to that address
    /// again if it was in the validators list.
    fn remove_peer_with_addr(&mut self, key: PublicKey) {
        self.state.remove_peer_with_pubkey(&key);

        let fork = self.blockchain.fork();
        NodeSchema::new(&fork).remove_peer_with_pubkey(&key);
        self.blockchain
            .merge(fork.into_patch())
            .expect("Cannot remove peer from peer cache");

        let is_validator = self.state.peer_is_validator(&key);
        let in_connect_list = self.state.peer_in_connect_list(&key);
        if is_validator && in_connect_list {
            self.connect(key);
        }
    }

    /// Handles the `Connect` message and connects to a peer as result.
    pub(crate) fn handle_connect(&mut self, message: Verified<Connect>) {
        // TODO Add spam protection (ECR-170)
        // TODO: drop connection if checks have failed. (ECR-1837)
        let address = message.payload().host.clone();
        if address == self.state.our_connect_message().payload().host {
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
            if saved_message.payload().time() > message.payload().time() {
                error!("Received outdated Connect message from {}", address);
                return;
            } else if saved_message.payload().time() < message.payload().time() {
                need_connect = saved_message.payload().host != message.payload().host;
            } else if saved_message.payload().host == message.payload().host {
                need_connect = false;
            } else {
                error!("Received weird Connect message from {}", address);
                return;
            }
            if saved_message.payload().host != message.payload().host {
                info!(
                    "Updating connect list for peer: {} with new addr: {}",
                    public_key,
                    message.payload().host
                );
                self.state
                    .connect_list()
                    .update_peer(&public_key, message.payload().host.to_string())
            }
        }

        self.state.add_peer(public_key, message.clone());
        info!(
            "Received Connect message from {}. Need to connect: {}",
            address, need_connect,
        );
        let fork = self.blockchain.fork();
        NodeSchema::new(&fork).save_peer(&public_key, message);
        self.blockchain
            .merge(fork.into_patch())
            .expect("Cannot save `Connect` from peer");

        if need_connect {
            // TODO: reduce double sending of connect message
            info!("Send Connect message to {}", address);
            //TODO: remove responding connect [ECR-2385]
            self.connect(public_key);
        }
    }

    /// Handles the `Status` message. Node sends `BlockRequest` as response if height in the
    /// message is higher than node's height.
    pub(crate) fn handle_status(&mut self, msg: &Verified<Status>) {
        let epoch = self.state.epoch();
        let block_height = self.state.blockchain_height();
        trace!(
            "HANDLE STATUS: current epoch / height = {} / {}, msg epoch / height = {} / {}",
            epoch,
            block_height,
            msg.payload().epoch,
            msg.payload().blockchain_height
        );

        if !self.state.connect_list().is_peer_allowed(&msg.author()) {
            error!(
                "Received status message from peer = {:?} which not in ConnectList.",
                msg.author()
            );
            return;
        }

        let peer = msg.author();
        let peer_state = PeerState::new(msg.payload());
        self.state.update_peer_state(peer, peer_state);

        // Handle message from future epoch / height.
        if peer_state.blockchain_height > block_height {
            // Request a block with the next height.
            self.request(RequestData::Block(block_height), peer);
        } else if peer_state.epoch > epoch {
            // Request a block with the next height or a block skip with a larger epoch.
            let data = RequestData::BlockOrEpoch {
                block_height,
                epoch,
            };
            self.request(data, peer);
        }

        if self.uncommitted_txs_count() == 0 && msg.payload().pool_size > 0 {
            self.request(RequestData::PoolTransactions, peer);
        }
    }

    /// Handles the `PeersRequest` message. Node sends `Connect` messages of other peers as result.
    pub(crate) fn handle_request_peers(&mut self, msg: &Verified<PeersRequest>) {
        let peers = self.state.peers().values().cloned().collect::<Vec<_>>();

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
    pub(crate) fn handle_status_timeout(&mut self, epoch: Height) {
        if self.state.epoch() == epoch {
            self.broadcast_status();
            self.add_status_timeout();
        }
    }
    /// Handles `NodeTimeout::PeerExchange`. Node sends the `PeersRequest` to a random peer.
    pub(crate) fn handle_peer_exchange_timeout(&mut self) {
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
                .nth(gen_peer_id())
                .map(|x| x.1.clone())
                .unwrap();
            let msg = PeersRequest::new(peer.author());
            trace!(
                "Request peers from peer with addr {:?}",
                peer.payload().host
            );
            let message = self.sign_message(msg);
            self.send_to_peer(peer.author(), message);
        }
        self.add_peer_exchange_timeout();
    }

    /// Handles `NodeTimeout::UpdateApiState`.
    /// Node update internal `ApiState` and `NodeRole`.
    pub(crate) fn handle_update_api_state_timeout(&mut self) {
        self.api_state.update_node_state(&self.state);
        // FIXME Add special event to update state [ECR-3222]
        self.node_role = NodeRole::new(self.state.validator_id());
        self.add_update_api_state_timeout();
    }

    /// Broadcasts the `Status` message to all peers.
    pub(crate) fn broadcast_status(&mut self) {
        let status = Status {
            epoch: self.state.epoch(),
            blockchain_height: self.state.blockchain_height(),
            last_hash: self.blockchain.as_ref().last_hash(),
            pool_size: self.uncommitted_txs_count(),
        };
        trace!("Broadcast status: {:?}", status);

        let message = self.sign_message(status);
        self.broadcast(message);
    }
}
