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
    pub fn handle_request(&mut self, msg: RequestMessage) {
        // Request are sended to us
        if msg.to() != self.state.consensus_public_key() {
            return;
        }

        if !self.state.whitelist().allow(msg.from()) {
            error!(
                "Received request message from peer = {:?} which not in whitelist.",
                msg.from()
            );
            return;
        }

        if !msg.verify(msg.from()) {
            error!("Received request with incorrect signature, msg={:?}", msg);
            return;
        }

        match msg {
            RequestMessage::Propose(msg) => self.handle_request_propose(msg),
            RequestMessage::Transactions(msg) => self.handle_request_txs(msg),
            RequestMessage::Prevotes(msg) => self.handle_request_prevotes(msg),
            RequestMessage::Peers(msg) => self.handle_request_peers(msg),
            RequestMessage::Block(msg) => self.handle_request_block(msg),
        }
    }

    /// Handles `ProposeRequest` message. For details see the message documentation.
    pub fn handle_request_propose(&mut self, msg: ProposeRequest) {
        trace!("HANDLE PROPOSE REQUEST");
        if msg.height() != self.state.height() {
            return;
        }

        let propose = if msg.height() == self.state.height() {
            self.state.propose(msg.propose_hash()).map(|p| {
                p.message().raw().clone()
            })
        } else {
            return;
        };

        if let Some(propose) = propose {
            self.send_to_peer(*msg.from(), &propose);
        }
    }

    /// Handles `TransactionsRequest` message. For details see the message documentation.
    pub fn handle_request_txs(&mut self, msg: TransactionsRequest) {
        trace!("HANDLE TRANSACTIONS REQUEST");
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
        }
    }

    /// Handles `PrevotesRequest` message. For details see the message documentation.
    pub fn handle_request_prevotes(&mut self, msg: PrevotesRequest) {
        trace!("HANDLE PREVOTES REQUEST");
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

        for prevote in &prevotes {
            self.send_to_peer(*msg.from(), prevote);
        }
    }

    /// Handles `BlockRequest` message. For details see the message documentation.
    pub fn handle_request_block(&mut self, msg: BlockRequest) {
        trace!(
            "Handle block request with height:{}, our height: {}",
            msg.height(),
            self.state.height()
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
