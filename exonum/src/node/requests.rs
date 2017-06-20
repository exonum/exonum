use messages::{RequestMessage, Message, RequestPropose, RequestTransactions, RequestPrevotes,
               RequestBlock, Block};
use blockchain::Schema;
use storage::Map;
use events::Channel;
use super::{NodeHandler, ExternalMessage, NodeTimeout};

// TODO: height should be updated after any message, not only after status (if signature is correct).
// TODO: Request propose makes sense only if we know that node is on our height.

impl<S> NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    pub fn handle_request(&mut self, msg: RequestMessage) {
        // Request are sended to us
        if msg.to() != self.state.public_key() {
            return;
        }

        if !self.state.whitelist().allow(msg.from()) {
            error!("Received request message from peer = {:?} which not in whitelist.", msg.from());
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

    pub fn handle_request_propose(&mut self, msg: RequestPropose) {
        trace!("HANDLE PROPOSE REQUEST!!!");
        if msg.height() != self.state.height() {
            return;
        }

        let propose = if msg.height() == self.state.height() {
            self.state.propose(msg.propose_hash()).map(|p| p.message().raw().clone())
        } else {
            return;
        };

        if let Some(propose) = propose {
            self.send_to_peer(*msg.from(), &propose);
        }
    }

    pub fn handle_request_txs(&mut self, msg: RequestTransactions) {
        trace!("HANDLE TRANSACTIONS REQUEST!!!");
        let view = self.blockchain.view();
        let schema = Schema::new(&view);
        for hash in msg.txs() {
            let tx = self.state
                .transactions()
                .get(hash)
                .map(|tx| tx.raw())
                .cloned()
                .or_else(|| schema.transactions().get(hash).unwrap());

            if let Some(tx) = tx {
                self.send_to_peer(*msg.from(), &tx);
            }
        }
    }

    pub fn handle_request_prevotes(&mut self, msg: RequestPrevotes) {
        trace!("HANDLE PREVOTES REQUEST!!!");
        if msg.height() != self.state.height() {
            return;
        }

        let has_prevotes = msg.validators();
        let prevotes = self.state
            .prevotes(msg.round(), *msg.propose_hash())
            .iter()
            .filter(|p| !has_prevotes[p.validator() as usize])
            .map(|p| p.raw().clone())
            .collect::<Vec<_>>();

        for prevote in &prevotes {
            self.send_to_peer(*msg.from(), prevote);
        }
    }

    pub fn handle_request_block(&mut self, msg: RequestBlock) {
        trace!("Handle block request with height:{}, our height: {}",
               msg.height(),
               self.state.height());
        if msg.height() >= self.state.height() {
            return;
        }

        let view = self.blockchain.view();
        let schema = Schema::new(&view);

        let height = msg.height();
        let block_hash = schema.block_hash_by_height(height).unwrap().unwrap();

        let block = schema.blocks().get(&block_hash).unwrap().unwrap();
        let precommits = schema.precommits(&block_hash)
            .values()
            .unwrap()
            .to_vec();
        let transactions = schema.block_txs(height)
            .values()
            .unwrap()
            .iter()
            .map(|tx_hash| schema.transactions().get(tx_hash).unwrap().unwrap())
            .collect::<Vec<_>>();

        let block_msg = Block::new(self.state.public_key(),
                                   msg.from(),
                                   block,
                                   precommits,
                                   transactions,
                                   self.state.secret_key());
        self.send_to_peer(*msg.from(), block_msg.raw());
    }
}
