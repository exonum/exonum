use super::super::storage::{Blockchain, Storage};
use super::super::messages::{Connect, Status, Message};
use super::{Node, RequestData};

impl<B: Blockchain> Node<B> {
    pub fn handle_connect(&mut self, message: Connect) {
        let public_key = message.pub_key().clone();
        let address = message.addr();
        if self.state.add_peer(public_key, address) {
            // TODO: reduce double sending of connect message
            info!("Establish connection with {}", address);
            let message = Connect::new(&self.public_key,
                                       self.events.address().clone(),
                                       self.events.get_time(),
                                       &self.secret_key);
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
                Some(public_key) => if !msg.verify(&public_key) {
                    return
                },
                // Incorrect validator id
                None => return
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
}
