use std::collections::HashSet;

use time::Duration;

use super::super::crypto::{Hash, PublicKey};
use super::super::blockchain::{Blockchain, View};
use super::super::messages::{ConsensusMessage, Propose, Prevote, Precommit, Message,
                             RequestPropose, RequestTransactions, RequestPrevotes,
                             RequestPrecommits, RequestBlock, Block};
use super::super::storage::Map;
use super::{NodeHandler, Round, Height, RequestData, ValidatorId};

use super::super::events::Channel;
use super::{ExternalMessage, NodeTimeout};

// TODO reduce view invokations
impl<B, S> NodeHandler<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout> + Clone
{
    pub fn handle_consensus(&mut self, msg: ConsensusMessage) {
        trace!("Consenus message with height {}, our height: {}", msg.height(), self.state.height());
        // Ignore messages from previous and future height
        if msg.height() < self.state.height() || msg.height() > self.state.height() + 1 {
            return;
        }

        // Queued messages from next height or round
        // TODO: shoud we ignore messages from far rounds?
        if msg.height() == self.state.height() + 1 || msg.round() > self.state.round() {
            self.state.add_queued(msg);
            return;
        }

        match self.state.public_key_of(msg.validator()) {
            // Incorrect signature of message
            Some(public_key) => {
                if !msg.verify(public_key) {
                    return;
                }
            }
            // Incorrect validator id
            None => return,
        }

        match msg {
            ConsensusMessage::Propose(msg) => {
                // Check prev_hash
                if msg.prev_hash() != self.state.last_hash() {
                    return;
                }

                // Check leader
                if msg.validator() != self.state.leader(msg.round()) {
                    return;
                }

                // Check timeout
                if msg.time() - self.last_block_time() <
                   Duration::milliseconds(self.propose_timeout as i64) {
                    return;
                }

                self.handle_propose(msg)
            }
            ConsensusMessage::Prevote(msg) => self.handle_prevote(msg),
            ConsensusMessage::Precommit(msg) => self.handle_precommit(msg),
        }
    }

    pub fn handle_propose(&mut self, msg: Propose) {
        debug!("Handle propose {:?}", msg);
        // TODO: check time
        // TODO: check that transactions are not commited yet
        if self.state.propose(&msg.hash()).is_some() {
            return;
        }

        let view = self.blockchain.view();
        for hash in msg.transactions() {
            if view.transactions().get(hash).unwrap().is_some() {
                return;
            }
        }

        // Add propose
        let (hash, has_unknown_txs) = match self.state.add_propose(msg.clone()) {
            Some(state) => (state.hash(), state.has_unknown_txs()),
            None => return,
        };

        // Remove request info
        let known_nodes = self.remove_request(RequestData::Propose(hash));

        if has_unknown_txs {
            debug!("REQUEST TRANSACTIONS!!!");
            let key = self.public_key_of(msg.validator());
            self.request(RequestData::Transactions(hash), key);
            for node in known_nodes {
                self.request(RequestData::Transactions(hash), node);
            }
        } else {
            self.has_full_propose(hash, msg.round());
        }
    }

    // TODO write helper function which returns Result
    pub fn handle_block(&mut self, msg: Block) {
        // TODO
        // Добавить pub_key получателя и время отправления, чтобы защититься от ддоса

        if !msg.verify(msg.from()) {
            return;
        }
        debug!("Handle block {:?}", msg);

        let block = msg.block();
        let block_hash = block.hash();
        // TODO add block with greater height to queue
        if self.state.height() != block.height() {
            return;
        }

        if block.prev_hash() != &self.last_block_hash() {
            return;
        }

        // Verify transactions
        let mut txs = Vec::new();
        for raw in msg.transactions() {
            match B::Transaction::from_raw(raw) {
                Ok(tx) => {
                    if !B::verify_tx(&tx) {
                        return;
                    }
                    txs.push((tx.hash(), tx));
                }
                Err(_) => return,
            }
        }
        // Verify precommits and adds them to state
        let precommits = msg.precommits();
        let mut has_consensus = false;
        for precommit in &precommits {
            if let Some(pub_key) = self.state.public_key_of(precommit.validator()) {
                if !precommit.verify(pub_key) || precommit.block_hash() != &block_hash {
                    warn!("Block with incorrect precommit received from {:?}", msg.from());
                    return;
                }
            } else {
                return;
            }
            has_consensus = self.state.add_precommit(&precommit);
        }
        // Ensure that we have consensus for this block
        if !has_consensus {
            return;
        }

        let (block_hash, patch) =
            self.blockchain
                .create_patch(block.height(),
                              block.time(),
                              block.proposer(),
                              txs.as_slice())
                .unwrap();
        // Verify block_hash
        if block_hash != block.hash() {
            warn!("Block with incorrect hash received from {:?}", msg.from());
            return;
        }
        // Commit block
        self.state.add_block(block_hash, patch);
        self.commit(block_hash, precommits.iter());
        // Request next block if needed
        let heights = self.state.validator_heights();
        if !heights.is_empty() {
            for id in heights {
                let peer = self.state.public_key_of(id).unwrap().clone();
                if self.state.peers().contains_key(&peer) {
                    let height = self.state.height();
                    self.request(RequestData::Block(height), peer);
                    info!("Send block request to peer: {} height: {}", id, height);
                    break;
                }
            }
        } 
    }

    pub fn has_full_propose(&mut self, hash: Hash, propose_round: Round) {
        // Send prevote
        if self.state.locked_round() == 0 {
            // TODO: what if we HAVE prevote for the propose round?
            if !self.state.have_prevote(propose_round) {
                self.send_prevote(propose_round, &hash);
            }
        }

        // Lock to propose
        // TODO: avoid loop here
        let start_round = ::std::cmp::max(self.state.locked_round() + 1, propose_round);
        for round in start_round...self.state.round() {
            if self.state.has_majority_prevotes(round, hash) {
                self.lock(round, hash);
            }
        }

        // Commit propose
        for (round, block_hash) in self.state.unknown_propose_with_precommits(&hash) {
            // Execute block and get state hash
            let our_block_hash = self.execute(&hash);

            if our_block_hash != block_hash {
                panic!("We are fucked up...");
            }

            let precommits = self.state.precommits(round, our_block_hash).unwrap()
                .iter()
                .map(|(_, x)| x.clone())
                .collect::<Vec<_>>();
            self.commit(our_block_hash, precommits.iter());
        }
    }

    pub fn handle_prevote(&mut self, prevote: Prevote) {
        debug!("Handle prevote {:?}", prevote);
        // Add prevote
        let has_consensus = self.state.add_prevote(&prevote);

        // Request propose or transactions
        self.request_propose_or_txs(prevote.propose_hash(), prevote.validator());

        // Request prevotes
        if prevote.locked_round() > self.state.locked_round() {
            let key = self.public_key_of(prevote.validator());
            self.request(RequestData::Prevotes(prevote.locked_round(), *prevote.propose_hash()),
                         key);
        }

        // Lock to propose
        if has_consensus {
            self.has_majority_prevotes(prevote.round(), prevote.propose_hash());
        }
    }

    pub fn has_majority_prevotes(&mut self, round: Round, propose_hash: &Hash) {
        // Remove request info
        self.remove_request(RequestData::Prevotes(round, *propose_hash));
        // Lock to propose
        if self.state.locked_round() < round {
            // FIXME: проверка что у нас есть все транзакции
            if self.state.propose(propose_hash).is_some() {
                self.lock(round, *propose_hash);
            }
        }
    }

    pub fn has_majority_precommits(&mut self,
                                   round: Round,
                                   propose_hash: &Hash,
                                   block_hash: &Hash) {
        // Remove request info
        self.remove_request(RequestData::Precommits(round, *propose_hash, *block_hash));
        // Commit
        if self.state.propose(propose_hash).is_some() {
            // FIXME: проверка что у нас есть все транзакции

            // Execute block and get state hash
            let our_block_hash = self.execute(propose_hash);

            if &our_block_hash != block_hash {
                panic!("We are fucked up...");
            }

            let precommits = self.state.precommits(round, our_block_hash).unwrap()
                .iter()
                .map(|(_, x)| x.clone())
                .collect::<Vec<_>>();
            self.commit(our_block_hash, precommits.iter());
        } else {
            self.state.add_unknown_propose_with_precommits(round, *propose_hash, *block_hash);
        }
    }

    pub fn lock(&mut self, round: Round, propose_hash: Hash) {
        debug!("MAKE LOCK {:?} {:?}", round, propose_hash);
        // Change lock
        self.state.lock(round, propose_hash);

        // Send precommit
        if !self.state.have_incompatible_prevotes() {
            // Execute block and get state hash
            let block_hash = self.execute(&propose_hash);
            self.send_precommit(round, &propose_hash, &block_hash);
            // Commit if has consensus
            if self.state.has_majority_precommits(round, block_hash) {
                self.has_majority_precommits(round, &propose_hash, &block_hash);
                return;
            }
        }

        // Send prevotes
        for round in self.state.locked_round() + 1...self.state.round() {
            if !self.state.have_prevote(round) {
                self.send_prevote(round, &propose_hash);
                if self.state.has_majority_prevotes(round, propose_hash) {
                    self.has_majority_prevotes(round, &propose_hash);
                }
            }
        }
    }

    pub fn handle_precommit(&mut self, msg: Precommit) {
        debug!("Handle precommit {:?}", msg);
        // Add precommit
        let has_consensus = self.state.add_precommit(&msg);

        let peer = self.public_key_of(msg.validator());
        // Request propose
        if let None = self.state.propose(msg.propose_hash()) {
            self.request(RequestData::Propose(*msg.propose_hash()), peer);
        }

        // Request prevotes
        // FIXME: если отправитель precommit находится на бОльшей высоте,
        // у него уже нет +2/3 prevote. Можем ли мы избавится от бесполезной
        // отправки RequestPrevotes?
        if msg.round() > self.state.locked_round() {
            self.request(RequestData::Prevotes(msg.round(), *msg.propose_hash()),
                         peer);
        }

        // Has majority precommits
        if has_consensus {
            self.has_majority_precommits(msg.round(), msg.propose_hash(), msg.block_hash());
        }
    }

    // FIXME: push precommits into storage
    pub fn commit<'a, I: Iterator<Item = &'a Precommit>>(&mut self, block_hash: Hash, precommits: I) {
        debug!("COMMIT {:?}", block_hash);
        // Merge changes into storage
        {
            // FIXME: remove unwrap here
            let patch = self.state.block(&block_hash).unwrap().patch();
            self.blockchain.commit(block_hash, patch, precommits).unwrap();
        }

        // Update state to new height
        let round = self.actual_round();
        // FIXME rewrite me!
        let txs = self.blockchain.view().block_txs(self.state.height()).values().unwrap();
        self.state.new_height(&block_hash, round, &txs);

        info!("{:?} ========== height = {}, commited = {}, pool = {}",
              self.channel.get_time(),
              self.state.height(),
              self.state.commited_txs,
              self.state.transactions().len());

        // Handle queued messages
        for msg in self.state.queued() {
            self.handle_consensus(msg);
        }

        // Send propose
        if self.is_leader() {
            self.add_propose_timeout();
        }

        // Add timeout for first round
        self.add_round_timeout();
    }

    pub fn handle_tx(&mut self, msg: B::Transaction) {
        debug!("Handle tx {:?}", msg);
        let hash = Message::hash(&msg);

        // Make sure that it is new transaction
        // TODO: use contains instead of get?
        if self.state.transactions().contains_key(&hash) {
            return;
        }

        let view = self.blockchain.view();
        if view.transactions().get(&hash).unwrap().is_some() {
            return;
        }

        if !B::verify_tx(&msg) {
            return;
        }


        let full_proposes = self.state.add_transaction(hash, msg);

        // Go to has full propose if we get last transaction
        for (hash, round) in full_proposes {
            self.remove_request(RequestData::Transactions(hash));
            self.has_full_propose(hash, round);
        }
    }

    pub fn handle_incoming_tx(&mut self, msg: B::Transaction) {
        debug!("Handle incoming tx {:?}", msg);
        let hash = Message::hash(&msg);

        // Make sure that it is new transaction
        // TODO: use contains instead of get?
        if self.state.transactions().contains_key(&hash) {
            return;
        }

        let view = self.blockchain.view();
        if view.transactions().get(&hash).unwrap().is_some() {
            return;
        }

        let full_proposes = self.state.add_transaction(hash, msg.clone());
        // Go to has full propose if we get last transaction
        for (hash, round) in full_proposes {
            self.remove_request(RequestData::Transactions(hash));
            self.has_full_propose(hash, round);
        }

        // broadcast transaction to validators
        self.broadcast(msg.raw());
    }

    pub fn handle_round_timeout(&mut self, height: Height, round: Round) {
        if height != self.state.height() {
            return;
        }
        if round != self.state.round() {
            return;
        }
        debug!("ROUND TIMEOUT height={}, round={}", height, round);

        // Update state to new round
        self.state.new_round();

        // Add timeout for this round
        self.add_round_timeout();

        // Send prevote if we are locked or propose if we are leader
        if let Some(hash) = self.state.locked_propose() {
            let round = self.state.round();
            self.send_prevote(round, &hash);
        } else if self.is_leader() {
            self.add_propose_timeout();
        }

        // Handle queued messages
        for msg in self.state.queued() {
            self.handle_consensus(msg);
        }
    }

    pub fn handle_propose_timeout(&mut self) {
        debug!("I AM LEADER!!! pool = {}", self.state.transactions().len());
        let round = self.state.round();
        let txs: Vec<Hash> = self.state
            .transactions()
            .keys()
            .cloned()
            .collect();
        let propose = Propose::new(self.state.id(),
                                   self.state.height(),
                                   round,
                                   self.channel.get_time(),
                                   self.state.last_hash(),
                                   &txs,
                                   &self.secret_key);
        self.broadcast(propose.raw());
        debug!("Send propose: {:?}", propose);

        // Save our propose into state
        let hash = self.state.add_self_propose(propose);

        // Send prevote
        self.send_prevote(round, &hash);
    }

    pub fn handle_request_timeout(&mut self, data: RequestData, peer: Option<PublicKey>) {
        debug!("!!!!!!!!!!!!!!!!!!! HANDLE REQUEST TIMEOUT");
        // FIXME: check height?
        if let Some(peer) = self.state.retry(&data, peer) {
            self.add_request_timeout(data.clone(), Some(peer));

            let message = match data {
                RequestData::Propose(ref propose_hash) => {
                    RequestPropose::new(&self.public_key,
                                        &peer,
                                        self.channel.get_time(),
                                        self.state.height(),
                                        propose_hash,
                                        &self.secret_key)
                        .raw()
                        .clone()
                }
                RequestData::Transactions(ref propose_hash) => {
                    let txs: Vec<_> = self.state
                        .propose(propose_hash)
                        .unwrap()
                        .unknown_txs()
                        .iter()
                        .cloned()
                        .collect();
                    RequestTransactions::new(&self.public_key,
                                             &peer,
                                             self.channel.get_time(),
                                             &txs,
                                             &self.secret_key)
                        .raw()
                        .clone()
                }
                RequestData::Prevotes(round, ref propose_hash) => {
                    RequestPrevotes::new(&self.public_key,
                                         &peer,
                                         self.channel.get_time(),
                                         self.state.height(),
                                         round,
                                         propose_hash,
                                         &self.secret_key)
                        .raw()
                        .clone()
                }
                RequestData::Precommits(round, ref propose_hash, ref block_hash) => {
                    RequestPrecommits::new(&self.public_key,
                                           &peer,
                                           self.channel.get_time(),
                                           self.state.height(),
                                           round,
                                           propose_hash,
                                           block_hash,
                                           &self.secret_key)
                        .raw()
                        .clone()
                }
                RequestData::Block(height) => {
                    RequestBlock::new(&self.public_key,
                                      &peer,
                                      self.channel.get_time(),
                                      height,
                                      &self.secret_key)
                        .raw()
                        .clone()
                }
            };
            self.send_to_peer(peer, &message);
            debug!("!!!!!!!!!!!!!!!!!!! Send request {:?} to peer {:?}",
                   message,
                   peer);
        }
    }

    // TODO: move this to state
    pub fn is_leader(&self) -> bool {
        self.state.leader(self.state.round()) == self.state.id()
    }

    // FIXME: remove this bull shit
    pub fn execute(&mut self, propose_hash: &Hash) -> Hash {
        let propose = self.state.propose(propose_hash).unwrap().message().clone();
        let (block_hash, patch) = {
            let txs = propose.transactions()
                .iter()
                .map(|propose_hash| {
                    let tx = self.state.transactions().get(propose_hash).unwrap();
                    (*propose_hash, tx.clone())
                })
                .collect::<Vec<_>>();
            self.blockchain
                .create_patch(propose.height(),
                              propose.time(),
                              propose.validator(),
                              txs.as_slice())
                .unwrap()
        };
        // Save patch
        self.state.add_block(block_hash, patch);
        block_hash
    }

    pub fn request_propose_or_txs(&mut self, propose_hash: &Hash, validator: ValidatorId) {
        let requested_data = match self.state.propose(propose_hash) {
            Some(state) => {
                // Request transactions
                if state.has_unknown_txs() {
                    Some(RequestData::Transactions(*propose_hash))
                } else {
                    None
                }
            }
            None => {
                // Request propose
                Some(RequestData::Propose(*propose_hash))
            }
        };

        if let Some(data) = requested_data {
            let key = self.public_key_of(validator);
            self.request(data, key);
        }
    }

    pub fn remove_request(&mut self, data: RequestData) -> HashSet<PublicKey> {
        // TODO: clear timeout
        self.state.remove_request(&data)
    }

    pub fn send_prevote(&mut self, round: Round, propose_hash: &Hash) {
        let locked_round = self.state.locked_round();
        let prevote = Prevote::new(self.state.id(),
                                   self.state.height(),
                                   round,
                                   propose_hash,
                                   locked_round,
                                   &self.secret_key);
        let has_majority_prevotes = self.state.add_prevote(&prevote);
        self.broadcast(prevote.raw());
        debug!("Send prevote: {:?}", prevote);
        if has_majority_prevotes {
            self.lock(round, *propose_hash);
        }
    }

    pub fn send_precommit(&mut self, round: Round, propose_hash: &Hash, block_hash: &Hash) {
        let precommit = Precommit::new(self.state.id(),
                                       self.state.height(),
                                       round,
                                       propose_hash,
                                       block_hash,
                                       &self.secret_key);
        self.state.add_precommit(&precommit);
        self.broadcast(precommit.raw());
        debug!("Send precommit: {:?}", precommit);
    }

    fn public_key_of(&self, id: ValidatorId) -> PublicKey {
        *self.state.public_key_of(id).unwrap()
    }
}
