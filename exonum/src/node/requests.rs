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

use super::NodeHandler;
use blockchain::Schema;
use messages::{
    BlockRequest, BlockResponse, Message, PrevotesRequest, ProposeRequest, Requests,
    TransactionsRequest, TransactionsResponse, RAW_TRANSACTION_HEADER,
    TRANSACTION_RESPONSE_EMPTY_SIZE,
};

// TODO: Height should be updated after any message, not only after status (if signature is correct). (ECR-171)
// TODO: Request propose makes sense only if we know that node is on our height. (ECR-171)

impl NodeHandler {
    /// Validates request, then redirects it to the corresponding `handle_...` function.
    pub fn handle_request(&mut self, msg: Requests) {
        // Request are sent to us
        if msg.to() != *self.state.consensus_public_key() {
            error!("Received message addressed to other peer = {:?}.", msg.to());
            return;
        }

        if !self.state.connect_list().is_peer_allowed(&msg.author()) {
            error!(
                "Received request message from peer = {:?} which not in ConnectList.",
                msg.author()
            );
            return;
        }

        match msg {
            Requests::ProposeRequest(ref msg) => self.handle_request_propose(msg),
            Requests::TransactionsRequest(ref msg) => self.handle_request_txs(msg),
            Requests::PrevotesRequest(ref msg) => self.handle_request_prevotes(msg),
            Requests::PeersRequest(ref msg) => self.handle_request_peers(msg),
            Requests::BlockRequest(ref msg) => self.handle_request_block(msg),
        }
    }

    /// Handles `ProposeRequest` message. For details see the message documentation.
    pub fn handle_request_propose(&mut self, msg: &Message<ProposeRequest>) {
        trace!("HANDLE PROPOSE REQUEST");
        if msg.height() != self.state.height() {
            return;
        }

        let propose = if msg.height() == self.state.height() {
            self.state
                .propose(msg.propose_hash())
                .map(|p| p.message().clone())
        } else {
            return;
        };

        if let Some(propose) = propose {
            self.send_to_peer(msg.author(), propose);
        }
    }

    /// Handles `TransactionsRequest` message. For details see the message documentation.
    pub fn handle_request_txs(&mut self, msg: &Message<TransactionsRequest>) {
        use std::mem;
        trace!("HANDLE TRANSACTIONS REQUEST");
        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        let mut txs = Vec::new();
        let mut txs_size = 0;
        let unoccupied_message_size = self.state.config().consensus.max_message_len as usize
            - TRANSACTION_RESPONSE_EMPTY_SIZE;

        for hash in msg.txs() {
            let tx = schema.transactions().get(hash);
            if let Some(tx) = tx {
                let raw = tx.signed_message().raw().to_vec();
                if txs_size + raw.len() + RAW_TRANSACTION_HEADER > unoccupied_message_size {
                    let txs_response = self.sign_message(TransactionsResponse::new(
                        &msg.author(),
                        mem::replace(&mut txs, vec![]),
                    ));

                    self.send_to_peer(msg.author(), txs_response);
                    txs_size = 0;
                }
                txs_size += raw.len() + RAW_TRANSACTION_HEADER;
                txs.push(raw);
            }
        }

        if !txs.is_empty() {
            let txs_response = self.sign_message(TransactionsResponse::new(&msg.author(), txs));

            self.send_to_peer(msg.author(), txs_response);
        }
    }

    /// Handles `PrevotesRequest` message. For details see the message documentation.
    pub fn handle_request_prevotes(&mut self, msg: & Message<PrevotesRequest>) {
        trace!("HANDLE PREVOTES REQUEST");
        if msg.height() != self.state.height() {
            return;
        }

        let has_prevotes = msg.validators();
        let prevotes = self.state
            .prevotes(msg.round(), *msg.propose_hash())
            .iter()
            .filter(|p| !has_prevotes[p.validator().into()])
            .cloned()
            .collect::<Vec<_>>();

        for prevote in prevotes {
            self.send_to_peer(msg.author(), prevote);
        }
    }

    /// Handles `BlockRequest` message. For details see the message documentation.
    pub fn handle_request_block(&mut self, msg: &Message<BlockRequest>) {
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
        let transactions = schema.block_transactions(height);

        let block_msg = self.sign_message(BlockResponse::new(
            &msg.author(),
            block,
            precommits
                .iter()
                .map(|p| p.signed_message().raw().to_vec())
                .collect(),
            &transactions.iter().collect::<Vec<_>>(),
        ));
        self.send_to_peer(msg.author(), block_msg);
    }
}
