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

use messages::{RequestMessage, Message, ProposeRequest, TransactionsRequest, PrevotesRequest,
               BlockRequest, BlockResponse};
use blockchain::Schema;
use super::NodeHandler;

// TODO: height should be updated after any message, not only after status (if signature is correct)
// TODO: Request propose makes sense only if we know that node is on our height.
// (ECR-171)

impl NodeHandler {
    /// Validates request, then redirects it to the corresponding `handle_...` function.
    pub fn handle_request(&mut self, message: RequestMessage) {
        let peer_logger = Logger::root(self.consensus_logger().to_erased(),
                                       o!("peer_public_key" => message.from());
        // Request are sended to us
        if message.to() != self.state.consensus_public_key() {
            error!(peer_logger,
                   "Received request message, that was addressed to other validator.";
                   "message_to" => message.to()
            );
            return;
        }

        if !self.state.whitelist().allow(message.from()) {
            error!(peer_logger,
                "Received request message.";
                "authorised" => false
            );
            return;
        }

        if !message.verify(message.from()) {
            error!(peer_logger,
                   "Received request with incorrect signature", "message" => message);
            return;
        }

<<<<<<< HEAD
        match msg {
            RequestMessage::Propose(msg) => self.handle_request_propose(&msg),
            RequestMessage::Transactions(msg) => self.handle_request_txs(&msg),
            RequestMessage::Prevotes(msg) => self.handle_request_prevotes(&msg),
            RequestMessage::Peers(msg) => self.handle_request_peers(&msg),
            RequestMessage::Block(msg) => self.handle_request_block(&msg),
=======
        match message {
            RequestMessage::Propose(message) => self.handle_request_propose(message),
            RequestMessage::Transactions(message) => self.handle_request_txs(message),
            RequestMessage::Prevotes(message) => self.handle_request_prevotes(message),
            RequestMessage::Peers(message) => self.handle_request_peers(message),
            RequestMessage::Block(message) => self.handle_request_block(message),
>>>>>>> df14cc09... Rewrite requests node module to contextual logger.
        }
    }

    /// Handles `ProposeRequest` message. For details see the message documentation.
<<<<<<< HEAD
    pub fn handle_request_propose(&mut self, msg: &ProposeRequest) {
        trace!("HANDLE PROPOSE REQUEST");
=======
    pub fn handle_request_propose(&mut self, msg: ProposeRequest) {
        trace!(self.consensus_logger(), "Handle propose request");
>>>>>>> df14cc09... Rewrite requests node module to contextual logger.
        if msg.height() != self.state.height() {
            trace!(self.consensus_logger(), "Received propose request from other height";
            "message_height" => msg.height());
            return;
        }

        let propose = self.state.propose(msg.propose_hash()).map(|p| {
            p.message().raw().clone()
        })


        if let Some(propose) = propose {
            self.send_to_peer(*msg.from(), &propose);
        }
        else {
            warn!(self.consensus_logger(), "Received propose request with unknown propose hash.");
        }
    }

    /// Handles `TransactionsRequest` message. For details see the message documentation.
<<<<<<< HEAD
    pub fn handle_request_txs(&mut self, msg: &TransactionsRequest) {
        trace!("HANDLE TRANSACTIONS REQUEST");
=======
    pub fn handle_request_txs(&mut self, msg: TransactionsRequest) {
        trace!(self.consensus_logger(), "Handle transaction request");
>>>>>>> df14cc09... Rewrite requests node module to contextual logger.
        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        for hash in msg.txs() {
            let tx = self.state
                .transactions()
                .read()
                .expect("Expected read lock")
                .get(hash)
                .map(|tx| tx.raw())
                .cloned()
                .or_else(|| schema.transactions().get(hash));

            if let Some(tx) = tx {
                self.send_to_peer(*msg.from(), &tx);
            }
            else {
                warn!(self.consensus_logger(), "Received transaction request with unknown tx hash.");
            }
        }
    }

    /// Handles `PrevotesRequest` message. For details see the message documentation.
<<<<<<< HEAD
    pub fn handle_request_prevotes(&mut self, msg: &PrevotesRequest) {
        trace!("HANDLE PREVOTES REQUEST");
=======
    pub fn handle_request_prevotes(&mut self, msg: PrevotesRequest) {
        trace!("Handle prevotes request");
>>>>>>> df14cc09... Rewrite requests node module to contextual logger.
        if msg.height() != self.state.height() {
            return;
        }

        let has_prevotes = msg.validators();
        let prevotes = self.state
            .prevotes(msg.round(), *msg.propose_hash())
            .iter()
            .filter(|p| !has_prevotes[p.validator().into()])
            .map(|p| p.raw().clone())
            .collect::<Vec<_>>();

        if prevotes.empty() {
            warn!(self.consensus_logger(), "Received prevotes request with unknown propose hash.");
        }

        for prevote in &prevotes {
            self.send_to_peer(*msg.from(), prevote);
        }
    }

    /// Handles `BlockRequest` message. For details see the message documentation.
    pub fn handle_request_block(&mut self, msg: &BlockRequest) {
        trace!(self.consensus_logger(),
            "Handle block request",
            msg.height(),
        );
        if msg.height() >= self.state.height() {
            return;
        }

        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);

        let height = msg.height();
        let block_hash = schema.block_hash_by_height(height).unwrap();

        let block = schema.blocks().get(&block_hash).unwrap();
        let precommits = schema.precommits(&block_hash);
        let transactions = schema.block_txs(height);


        let block_msg = BlockResponse::new(
            self.state.consensus_public_key(),
            msg.from(),
            block,
            precommits.iter().collect(),
            transactions
                .iter()
                .map(|tx_hash| schema.transactions().get(&tx_hash).unwrap())
                .collect(),
            self.state.consensus_secret_key(),
        );
        self.send_to_peer(*msg.from(), block_msg.raw());
    }
}
