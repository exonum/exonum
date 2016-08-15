extern crate rand;

use rand::Rng;

use super::super::storage::{Blockchain, BlockStorage};
use super::super::messages::{Connect, Status, Message, RequestPeers};
use super::{Node, RequestData};
use super::state::ValidatorId;

impl<B: Blockchain> Node<B> {
    pub fn handle_connect(&mut self, message: Connect) {
        // TODO add spam protection
        let address = message.addr();
        info!("recv connect message from {}", address);
        if address == self.state.our_connect_message().addr() {
            return;
        }

        // Check if we have another connect message from peer with the given public_key
        let public_key = *message.pub_key();
        let mut need_connect = true;
        if let Some(saved_message) = self.state.peers().get(&public_key) {
            if saved_message.time() > message.time() {
                info!("Received weird connection message from {}", address);
                return;
            }
            need_connect = !(saved_message.addr() == message.addr()
                && saved_message.time() == message.time());
        }
        self.state.add_peer(public_key, message);

        if need_connect {
            // TODO: reduce double sending of connect message
            info!("Establish connection with {}", address);
            let message = self.state.our_connect_message().clone();
            self.send_to_addr(&address, message.raw());
        }
    }

    pub fn handle_status(&mut self, msg: Status) {
        info!("recv status");
        // Handle message from future height
        if msg.height() > self.state.height() {
            // Check validator height info
            // FIXME: make sure that validator id < validator count
            if self.state.validator_height(msg.validator()) >= msg.height() {
                return;
            }
            // Verify validator if and signature
            match self.state.public_key_of(msg.validator()) {
                // Incorrect signature of message
                Some(public_key) => {
                    if !msg.verify(public_key) {
                        return;
                    }
                }
                // Incorrect validator id
                None => return,
            };
            // Update validator height
            self.state.set_validator_height(msg.validator(), msg.height());
            // Request commit
            self.request(RequestData::Commit, msg.validator());
        }

        // TODO: remove this?
        // // Handle message from current height
        // if msg.height() == self.state.height() {
        //     // Request propose or txs
        //     self.request_propose_or_txs(ctx, msg.propose_hash(), msg.validator());

        //     // Request precommits
        //     if !self.state.has_majority_precommits(msg.round(),
        //                                           *msg.propose_hash(),
        //                                           *msg.block_hash()) {
        //         let data = RequestData::Precommits(msg.round(),
        //                                           *msg.propose_hash(),
        //                                           *msg.block_hash());
        //         self.request(ctx, data, msg.validator());
        //     }
        // }
    }

    pub fn handle_request_peers(&mut self, msg: RequestPeers) {
        info!("recv peers request from validator {}", msg.from());
        let peers: Vec<Connect> = self.state.peers().iter().map(|(_, b)| b.clone()).collect();
        for peer in peers {
            self.send_to_validator(msg.from(), peer.raw());
        }
    }

    pub fn handle_status_timeout(&mut self) {
        if let Some(hash) = self.blockchain.last_hash().unwrap() {
            info!("send status");
            // Send status
            let status = Status::new(self.state.id(),
                                     self.state.height(),
                                     &hash,
                                     &self.secret_key);
            self.broadcast(status.raw());
        }
        self.add_status_timeout();
    }

    pub fn handle_peer_exchange_timeout(&mut self) {
        let to = self.state.validators().len() as ValidatorId - 1;
        let gen_validator_id = || {
            let mut rng = rand::thread_rng();
            rng.gen_range(0, to)
        };

        let mut validator = gen_validator_id();
        while validator == self.state.id() {
            validator = gen_validator_id();
        }

        let msg = RequestPeers::new(self.state.id(),
                                    validator,
                                    self.events.get_time(),
                                    &self.secret_key);
        self.send_to_validator(validator, msg.raw());

        info!("request peers from validator {}", validator);
        self.add_peer_exchange_timeout();
    }
}
