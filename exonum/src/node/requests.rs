use super::super::messages::{RequestMessage, Message, RequestPropose, RequestTransactions,
                             RequestPrevotes, RequestPrecommits, RequestCommit};
use super::super::blockchain::{Blockchain, TxStorage, BlockStorage};
use super::super::storage::{Map, List};
use super::Node;

const REQUEST_ALIVE: i64 = 3_000_000_000; // 3 seconds

impl<B: Blockchain> Node<B> {
    pub fn handle_request(&mut self, msg: RequestMessage) {
        // Request are sended to us
        if msg.to() != &self.public_key {
            return;
        }

        // FIXME: we should use some epsilon for checking lifetime < 0
        let lifetime = match (self.events.get_time() - msg.time()).num_nanoseconds() {
            Some(nanos) => nanos,
            None => {
                // Incorrect time into message
                return;
            }
        };

        // Incorrect time of the request
        if lifetime < 0 || lifetime > REQUEST_ALIVE {
            return;
        }

        if !msg.verify(msg.from()) {
            return;
        }

        debug!("Handle request: {:?}", msg);
        match msg {
            RequestMessage::Propose(msg) => self.handle_request_propose(msg),
            RequestMessage::Transactions(msg) => self.handle_request_txs(msg),
            RequestMessage::Prevotes(msg) => self.handle_request_prevotes(msg),
            RequestMessage::Precommits(msg) => self.handle_request_precommits(msg),
            RequestMessage::Commit(msg) => self.handle_request_commit(msg),
            RequestMessage::Peers(msg) => self.handle_request_peers(msg),
        }
    }

    pub fn handle_request_propose(&mut self, msg: RequestPropose) {
        if msg.height() > self.state.height() {
            return;
        }

        let propose = if msg.height() == self.state.height() {
            self.state.propose(msg.propose_hash()).map(|p| p.message().raw().clone())
        } else {
            // msg.height < state.height
            self.blockchain.proposes().get(msg.propose_hash()).unwrap().map(|p| p.raw().clone())
        };

        if let Some(propose) = propose {
            self.send_to_peer(*msg.from(), &propose);
        }
    }

    pub fn handle_request_txs(&mut self, msg: RequestTransactions) {
        for hash in msg.txs() {
            let tx = self.state
                .transactions()
                .get(hash)
                .cloned()
                .or_else(|| self.blockchain.transactions().get(hash).unwrap());

            if let Some(tx) = tx {
                self.send_to_peer(*msg.from(), tx.raw());
            }
        }
    }

    pub fn handle_request_prevotes(&mut self, msg: RequestPrevotes) {
        if msg.height() != self.state.height() {
            return;
        }

        let prevotes = if let Some(prevotes) = self.state
            .prevotes(msg.round(), *msg.propose_hash()) {
            prevotes.values().map(|p| p.raw().clone()).collect()
        } else {
            Vec::new()
        };

        for prevote in prevotes {
            self.send_to_peer(*msg.from(), &prevote);
        }
    }

    pub fn handle_request_precommits(&mut self, msg: RequestPrecommits) {
        if msg.height() > self.state.height() {
            return;
        }

        let precommits = if msg.height() == self.state.height() {
            if let Some(precommits) = self.state
                .precommits(msg.round(), *msg.propose_hash(), *msg.block_hash()) {
                precommits.values().map(|p| p.raw().clone()).collect()
            } else {
                Vec::new()
            }
        } else {
            // msg.height < state.height
            if let Some(precommits) = self.blockchain.precommits(msg.block_hash()).iter().unwrap() {
                precommits.iter().map(|p| p.raw().clone()).collect()
            } else {
                Vec::new()
            }
        };

        for precommit in precommits {
            self.send_to_peer(*msg.from(), &precommit);
        }
    }

    pub fn handle_request_commit(&mut self, msg: RequestCommit) {
        if msg.height() >= self.state.height() {
            return;
        }

        let block_hash = self.blockchain.heights().get(msg.height()).unwrap().unwrap();

        let precommits =
            if let Some(precommits) = self.blockchain.precommits(&block_hash).iter().unwrap() {
                precommits.iter().map(|p| p.raw().clone()).collect()
            } else {
                Vec::new()
            };

        for precommit in precommits {
            self.send_to_peer(*msg.from(), &precommit);
        }
    }
}
