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
use crypto::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use failure;
use messages::{
    BlockRequest, BlockResponse, Message, PrevotesRequest, ProposeRequest, RequestMessage,
    TransactionsRequest, TransactionsResponse,
};

// TODO: height should be updated after any message, not only after status (if signature is correct)
// TODO: Request propose makes sense only if we know that node is on our height.
// (ECR-171)

impl NodeHandler {
    /// Validates request, then redirects it to the corresponding `handle_...` function.
    pub fn handle_request(&mut self, msg: Message<RequestMessage>) -> Result<(), failure::Error> {
        // Request are sent to us
        if msg.to() != self.state.consensus_public_key() {
            bail!("Received message addressed to other peer = {:?}.", msg.to());
        }

        if !self.state.whitelist().allow(msg.author()) {
            bail!(
                "Received request message from peer = {:?} which not in whitelist.",
                msg.author()
            );
        }

        let (msg, signed) = msg.into_parts();
        match msg {
            RequestMessage::Propose(msg) => {
                self.handle_request_propose(Message::from_parts(msg, signed)?)
            }
            RequestMessage::Transactions(msg) => {
                self.handle_request_txs(Message::from_parts(msg, signed)?)
            }
            RequestMessage::Prevotes(msg) => {
                self.handle_request_prevotes(Message::from_parts(msg, signed)?)
            }
            RequestMessage::Peers(msg) => {
                self.handle_request_peers(Message::from_parts(msg, signed)?)
            }
            RequestMessage::Block(msg) => {
                self.handle_request_block(Message::from_parts(msg, signed)?)
            }
        };
        Ok(())
    }

    /// Handles `ProposeRequest` message. For details see the message documentation.
    pub fn handle_request_propose(&mut self, msg: Message<ProposeRequest>) {
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
            self.send_to_peer(*msg.author(), propose);
        }
    }

    /// Handles `TransactionsRequest` message. For details see the message documentation.
    pub fn handle_request_txs(&mut self, msg: Message<TransactionsRequest>) {
        use std::mem;
        trace!("HANDLE TRANSACTIONS REQUEST");
        unimplemented!();
        /*let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);

        let mut txs = Vec::new();
        let mut txs_size = 0;
        let EMPTY_RESPONSE_SIZE: u32 = unimplemented!();
           // (HEADER_LENGTH + SIGNATURE_LENGTH + 2 * PUBLIC_KEY_LENGTH + 8) as u32;
        let unoccupied_message_size =
            self.state.config().consensus.max_message_len - EMPTY_RESPONSE_SIZE;

        for hash in msg.txs() {
            let tx = schema.transactions().get(hash);
            if let Some(tx) = tx {
                if txs_size + tx.raw().len() as u32 > unoccupied_message_size {
                    let txs_response = self.sign_message(TransactionsResponse::new(
                        msg.author(),
                        mem::replace(&mut txs, vec![]),
                    ));

                    self.send_to_peer(*msg.author(), txs_response.raw());
                    txs_size = 0;
                }
                txs_size += tx.raw().len() as u32;
                txs.push(tx);
            }
        }

        if !txs.is_empty() {
            let txs_response = self.sign_message(TransactionsResponse::new(
                msg.author(),
                txs,
            ));

            self.send_to_peer(*msg.author(), txs_response);
        }*/
    }

    /// Handles `PrevotesRequest` message. For details see the message documentation.
    pub fn handle_request_prevotes(&mut self, msg: Message<PrevotesRequest>) {
        trace!("HANDLE PREVOTES REQUEST");
        if msg.height() != self.state.height() {
            return;
        }

        let has_prevotes = msg.validators();
        let prevotes = self.state
            .prevotes(msg.round(), *msg.propose_hash())
            .iter()
            .filter(|p| !has_prevotes[p.validator().into()])
            .map(|p| p.clone())
            .collect::<Vec<_>>();

        for prevote in prevotes {
            self.send_to_peer(*msg.author(), prevote);
        }
    }

    /// Handles `BlockRequest` message. For details see the message documentation.
    pub fn handle_request_block(&mut self, msg: Message<BlockRequest>) {
        unimplemented!();
        /*
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
            msg.author(),
            block,
            precommits.iter().collect(),
            transactions
                .iter()
                .map(|tx_hash| schema.transactions().get(&tx_hash).unwrap())
                .collect(),
        ));
        self.send_to_peer(*msg.author(), block_msg);
        */
    }
}
