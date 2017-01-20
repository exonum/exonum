use std::collections::HashSet;

use time::{Duration, Timespec};

use super::super::crypto::{Hash, PublicKey, HexValue};
use super::super::blockchain::{Schema, Transaction};
use super::super::messages::{ConsensusMessage, Propose, Prevote, Precommit, Message,
                             RequestPropose, RequestTransactions, RequestPrevotes,
                             RequestPrecommits, RequestBlock, Block, RawTransaction};
use super::super::storage::{Map, Patch};
use super::{NodeHandler, Round, Height, RequestData, ValidatorId};

use super::super::events::Channel;
use super::{ExternalMessage, NodeTimeout};

const BLOCK_ALIVE: i64 = 3_000_000_000; // 3 seconds

// TODO reduce view invokations
impl<S> NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    pub fn handle_consensus(&mut self, msg: ConsensusMessage) {
        // Ignore messages from previous and future height
        if msg.height() < self.state.height() || msg.height() > self.state.height() + 1 {
            warn!("Received consensus message from other height: msg.height={}, self.height={}",
                  msg.height(),
                  self.state.height());
            return;
        }

        // Queued messages from next height or round
        // TODO: shoud we ignore messages from far rounds?
        if msg.height() == self.state.height() + 1 || msg.round() > self.state.round() {
            trace!("Received consensus message from future round: msg.height={}, msg.round={}, \
                    self.height={}, self.round={}",
                   msg.height(),
                   msg.round(),
                   self.state.height(),
                   self.state.round());
            self.state.add_queued(msg);
            return;
        }

        match self.state.public_key_of(msg.validator()) {
            // incorrect signature of message
            Some(public_key) => {
                if !msg.verify(public_key) {
                    error!("Received message with incorrect signature msg={:?}", msg);
                    return;
                }
            }
            // incorrect validator id
            None => {
                error!("Received message from incorrect msg={:?}", msg);
                return;
            }
        }

        trace!("Handle message={:?}", msg);
        match msg {
            ConsensusMessage::Propose(msg) => self.handle_propose(msg),
            ConsensusMessage::Prevote(msg) => self.handle_prevote(msg),
            ConsensusMessage::Precommit(msg) => self.handle_precommit(msg),
        }
    }

    pub fn handle_propose(&mut self, msg: Propose) {
        // Check prev_hash
        if msg.prev_hash() != self.state.last_hash() {
            error!("Received propose with wrong last_block_hash msg={:?}", msg);
            return;
        }

        // Check leader
        if msg.validator() != self.state.leader(msg.round()) {
            error!("Wrong propose leader detected: actual={}, expected={}",
                   msg.validator(),
                   self.state.leader(msg.round()));
            return;
        }

        // check time of the propose
        let round = msg.round();
        let start_time = self.round_start_time(round) +
                         Duration::milliseconds(self.adjusted_propose_timeout());
        let end_time = start_time + Duration::milliseconds(self.round_timeout());

        if msg.time() < start_time || msg.time() > end_time {
            error!("Received propose with wrong time, msg={:?}", msg);
            return;
        }

        let view = self.blockchain.view();
        // Check that transactions are not commited yet
        for hash in msg.transactions() {
            if Schema::new(&view).transactions().get(hash).unwrap().is_some() {
                error!("Received propose with already commited transaction, msg={:?}",
                       msg);
                return;
            }
        }

        if self.state.propose(&msg.hash()).is_some() {
            return;
        }

        trace!("Handle propose");
        // Add propose
        let (hash, has_unknown_txs) = match self.state.add_propose(msg.clone()) {
            Some(state) => (state.hash(), state.has_unknown_txs()),
            None => return,
        };

        // Remove request info
        let known_nodes = self.remove_request(RequestData::Propose(hash));

        if has_unknown_txs {
            trace!("REQUEST TRANSACTIONS!!!");
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
        // Request are sended to us
        if msg.to() != &self.public_key {
            error!("Received block that intended for another peer, to={}, from={}",
                   msg.to().to_hex(),
                   msg.from().to_hex());
            return;
        }
        // FIXME: we should use some epsilon for checking lifetime < 0
        let lifetime = match (self.channel.get_time() - msg.time()).num_nanoseconds() {
            Some(nanos) => nanos,
            None => {
                // incorrect time into message
                error!("Received block with incorrect time msg={:?}", msg);
                return;
            }
        };
        // check time of the bock
        if lifetime < 0 || lifetime > BLOCK_ALIVE {
            error!("Received block with incorrect lifetime={}, msg={:?}",
                   lifetime,
                   msg);
            return;
        }

        trace!("Handle block");

        let block = msg.block();
        let block_hash = block.hash();

        // TODO add block with greater height to queue
        if self.state.height() != block.height() {
            return;
        }

        // Check block content
        if block.prev_hash() != &self.last_block_hash() {
            error!("Weird block received, block={:?}", msg);
            return;
        }

        // Verify propose time
        let propose_round = block.propose_round();
        let start_time = self.round_start_time(propose_round) +
                         Duration::milliseconds(self.adjusted_propose_timeout());
        let end_time = start_time + Duration::milliseconds(self.round_timeout());
        if msg.time() < start_time || block.time() > end_time {
            error!("Received block with wrong propose time, block={:?}", msg);
            return;
        }

        // Verify precommits
        let precommits = msg.precommits();
        if precommits.len() < self.state.majority_count() ||
           precommits.len() > self.state.validators().len() {
            error!("Received block without consensus, block={:?}", msg);
            return;
        }
        let precommit_round = precommits[0].round();
        for precommit in &precommits {
            let r = self.verify_precommit(&block_hash, block.height(), precommit_round, precommit);
            if let Err(e) = r {
                error!("{}, block={:?}", e, msg);
                return;
            }
        }

        if self.state.block(&block_hash).is_none() {
            let view = &self.blockchain.view();
            let schema = Schema::new(view);
            // Verify transactions
            let mut txs = Vec::new();
            for raw in msg.transactions() {
                if let Some(tx) = self.blockchain.tx_from_raw(raw) {
                    let hash = tx.hash();
                    if schema.transactions().get(&hash).unwrap().is_some() {
                        error!("Received block with already commited transaction, block={:?}",
                               msg);
                        return;
                    }
                    if !tx.verify() {
                        error!("Incorrect transaction in block detected, block={:?}", msg);
                        return;
                    }
                    txs.push((hash, tx));
                } else {
                    error!("Unknown transaction in block detected, block={:?}", msg);
                    return;
                }
            }

            let (block_hash, txs, patch) = self.create_block(block.height(),
                                                             block.propose_round(),
                                                             block.time(),
                                                             txs.as_slice());
            // Verify block_hash
            if block_hash != block.hash() {
                panic!("Block_hash incorrect in received block={:?}", msg);
            }

            // Commit block
            self.state.add_block(block_hash, patch, txs, propose_round);
        }
        self.commit(block_hash, precommits.iter());
        self.request_next_block();
    }

    pub fn has_full_propose(&mut self, hash: Hash, propose_round: Round) {
        // Send prevote
        if self.state.locked_round() == 0 {
            if !self.state.have_prevote(propose_round) {
                self.broadcast_prevote(propose_round, &hash);
            } else {
                // TODO: what if we HAVE prevote for the propose round?
            }
        }

        // Lock to propose
        // TODO: avoid loop here
        let start_round = ::std::cmp::max(self.state.locked_round() + 1, propose_round);
        for round in start_round...self.state.round() {
            if self.state.has_majority_prevotes(round, hash) {
                self.has_majority_prevotes(round, &hash);
            }
        }

        // Commit propose
        for (round, block_hash) in self.state.unknown_propose_with_precommits(&hash) {
            // Execute block and get state hash
            let our_block_hash = self.execute(&hash);

            if our_block_hash != block_hash {
                panic!("We are fucked up...");
            }

            let precommits = self.state
                .precommits(round, our_block_hash)
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            self.commit(our_block_hash, precommits.iter());
        }
    }

    pub fn handle_prevote(&mut self, prevote: Prevote) {
        trace!("Handle prevote");
        // Add prevote
        let has_consensus = self.state.add_prevote(&prevote);

        // Request propose or transactions
        let has_propose_with_txs =
            self.request_propose_or_txs(prevote.propose_hash(), prevote.validator());

        // Request prevotes
        if prevote.locked_round() > self.state.locked_round() {
            let key = self.public_key_of(prevote.validator());
            self.request(RequestData::Prevotes(prevote.locked_round(), *prevote.propose_hash()),
                         key);
        }

        // Lock to propose
        if has_consensus && has_propose_with_txs {
            self.has_majority_prevotes(prevote.round(), prevote.propose_hash());
        }
    }

    pub fn has_majority_prevotes(&mut self, prevote_round: Round, propose_hash: &Hash) {
        // Remove request info
        self.remove_request(RequestData::Prevotes(prevote_round, *propose_hash));
        // Lock to propose
        if self.state.locked_round() < prevote_round && self.state.propose(propose_hash).is_some() {
            self.lock(prevote_round, *propose_hash);
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
            // Check for unknown txs
            let has_unknown_txs = {
                let state = self.state.propose(propose_hash).unwrap();
                if state.has_unknown_txs() {
                    Some(state.message().validator())
                } else {
                    None
                }
            };
            if let Some(validator) = has_unknown_txs {
                let data = RequestData::Transactions(*propose_hash);
                let key = self.public_key_of(validator);
                self.request(data, key);
                return;
            }

            // Execute block and get state hash
            let our_block_hash = self.execute(propose_hash);

            if &our_block_hash != block_hash {
                panic!("We are fucked up...");
            }

            let precommits = self.state
                .precommits(round, our_block_hash)
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            self.commit(our_block_hash, precommits.iter());
        } else {
            self.state.add_unknown_propose_with_precommits(round, *propose_hash, *block_hash);
        }
    }

    pub fn lock(&mut self, prevote_round: Round, propose_hash: Hash) {
        trace!("MAKE LOCK {:?} {:?}", prevote_round, propose_hash);
        for round in prevote_round...self.state.round() {
            // Send prevotes
            if !self.state.have_prevote(round) {
                self.broadcast_prevote(round, &propose_hash);
            }
            // Change lock
            if self.state.has_majority_prevotes(round, propose_hash) {
                self.state.lock(round, propose_hash);
                // Send precommit
                if !self.state.have_incompatible_prevotes() {
                    // Execute block and get state hash
                    let block_hash = self.execute(&propose_hash);
                    self.broadcast_precommit(round, &propose_hash, &block_hash);
                    // Commit if has consensus
                    if self.state.has_majority_precommits(round, block_hash) {
                        self.has_majority_precommits(round, &propose_hash, &block_hash);
                        return;
                    }
                }
                // Remove request info
                self.remove_request(RequestData::Prevotes(round, propose_hash));
            }
        }
    }

    pub fn handle_precommit(&mut self, msg: Precommit) {
        trace!("Handle precommit");
        // Add precommit
        let has_consensus = self.state.add_precommit(&msg);

        let peer = self.public_key_of(msg.validator());
        // Request propose
        if self.state.propose(msg.propose_hash()).is_none() {
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
    pub fn commit<'a, I: Iterator<Item = &'a Precommit>>(&mut self,
                                                         block_hash: Hash,
                                                         precommits: I) {
        trace!("COMMIT {:?}", block_hash);

        // Merge changes into storage
        let (propose_round, commited_txs, new_txs) = {
            let (txs_count, propose_round) = {
                let block_state = self.state.block(&block_hash).unwrap();
                (block_state.txs().len(), block_state.propose_round())
            };

            let txs = self.blockchain
                .commit(&mut self.state, block_hash, precommits)
                .unwrap();

            (propose_round, txs_count, txs)
        };

        for tx in new_txs {
            assert!(tx.verify());
            self.handle_incoming_tx(tx.clone());
        }

        let height = self.state.height();
        let proposer = self.state.leader(propose_round);

        // Update state to new height
        let round = self.actual_round();

        let view = self.blockchain.view();
        let schema = Schema::new(&view);
        let config = schema.get_configuration_at_height(height).unwrap();
        self.state.new_height(&block_hash, round, config);

        info!("COMMIT ====== height={}, round={}, proposer={}, commited={}, pool={}",
              height,
              propose_round,
              proposer,
              commited_txs,
              self.state.transactions().len());

        // Add timeout for first round
        self.add_round_timeout();
        // Send propose we is leader
        if self.is_leader() {
            self.add_propose_timeout();
        }

        // Handle queued messages
        for msg in self.state.queued() {
            self.handle_consensus(msg);
        }
    }

    pub fn handle_tx(&mut self, msg: RawTransaction) {
        trace!("Handle transaction");
        let hash = msg.hash();
        let tx = {
            let service_id = msg.service_id();
            if let Some(tx) = self.blockchain.tx_from_raw(msg) {
                tx
            } else {
                error!("Received transaction with unknown service_id={}",
                       service_id);
                return;
            }
        };

        // Make sure that it is new transaction
        if self.state.transactions().contains_key(&hash) {
            return;
        }

        let view = self.blockchain.view();
        if Schema::new(&view).transactions().get(&hash).unwrap().is_some() {
            return;
        }

        if !tx.verify() {
            return;
        }

        let full_proposes = self.state.add_transaction(hash, tx);
        // Go to has full propose if we get last transaction
        for (hash, round) in full_proposes {
            self.remove_request(RequestData::Transactions(hash));
            self.has_full_propose(hash, round);
        }
    }

    pub fn handle_incoming_tx(&mut self, msg: Box<Transaction>) {
        trace!("Handle incoming transaction");
        let hash = msg.hash();

        // Make sure that it is new transaction
        if self.state.transactions().contains_key(&hash) {
            return;
        }

        let view = self.blockchain.view();
        if Schema::new(&view).transactions().get(&hash).unwrap().is_some() {
            return;
        }

        // Broadcast transaction to validators
        trace!("Broadcast transactions: {:?}", msg.raw());
        self.broadcast(msg.raw());

        let full_proposes = self.state.add_transaction(hash, msg);
        // Go to has full propose if we get last transaction
        for (hash, round) in full_proposes {
            self.remove_request(RequestData::Transactions(hash));
            self.has_full_propose(hash, round);
        }
    }

    pub fn handle_round_timeout(&mut self, height: Height, round: Round) {
        // TODO debug asserts?
        if height != self.state.height() {
            return;
        }
        if round != self.state.round() {
            return;
        }
        warn!("ROUND TIMEOUT height={}, round={}", height, round);

        // Update state to new round
        self.state.new_round();

        // Add timeout for this round
        self.add_round_timeout();

        // Send prevote if we are locked or propose if we are leader
        if let Some(hash) = self.state.locked_propose() {
            let round = self.state.round();
            let has_majority_prevotes = self.broadcast_prevote(round, &hash);
            if has_majority_prevotes {
                self.has_majority_prevotes(round, &hash);
            }
        } else if self.is_leader() {
            self.add_propose_timeout();
        }

        // Handle queued messages
        for msg in self.state.queued() {
            self.handle_consensus(msg);
        }
    }

    pub fn handle_propose_timeout(&mut self, height: Height, round: Round) {
        // TODO debug asserts?
        if height != self.state.height() {
            // It is too late
            return;
        }
        if round != self.state.round() {
            return;
        }
        if self.state.locked_propose().is_some() {
            return;
        }
        if self.state.have_prevote(round) {
            return;
        }

        info!("I AM LEADER!!! pool = {}", self.state.transactions().len());

        let round = self.state.round();
        let max_count = ::std::cmp::min(self.txs_block_limit() as usize,
                                        self.state.transactions().len());
        let txs: Vec<Hash> = self.state
            .transactions()
            .keys()
            .take(max_count)
            .cloned()
            .collect();
        let propose = Propose::new(self.state.id(),
                                   self.state.height(),
                                   round,
                                   self.channel.get_time(),
                                   self.state.last_hash(),
                                   &txs,
                                   &self.secret_key);
        trace!("Broadcast propose: {:?}", propose);
        self.broadcast(propose.raw());

        // Save our propose into state
        let hash = self.state.add_self_propose(propose);

        // Send prevote
        let has_majority_prevotes = self.broadcast_prevote(round, &hash);
        if has_majority_prevotes {
            self.has_majority_prevotes(round, &hash);
        }
    }

    pub fn handle_request_timeout(&mut self, data: RequestData, peer: Option<PublicKey>) {
        trace!("!!!!!!!!!!!!!!!!!!! HANDLE REQUEST TIMEOUT");
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
                                         self.state.known_prevotes(round, propose_hash),
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
                                           self.state.known_precommits(round, propose_hash),
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
            trace!("!!!!!!!!!!!!!!!!!!! Send request {:?} to peer {:?}",
                   data,
                   peer);
            self.send_to_peer(peer, &message);
        }
    }

    // TODO: move this to state
    pub fn is_leader(&self) -> bool {
        self.state.leader(self.state.round()) == self.state.id()
    }

    pub fn create_block(&mut self,
                        height: Height,
                        round: Round,
                        time: Timespec,
                        txs: &[(Hash, Box<Transaction>)])
                        -> (Hash, Vec<Hash>, Patch) {
        self.blockchain
            .create_patch(height, round, time, txs)
            .unwrap()
    }

    // FIXME: remove this bull shit
    pub fn execute(&mut self, propose_hash: &Hash) -> Hash {
        let propose = self.state.propose(propose_hash).unwrap().message().clone();
        let txs = propose.transactions()
            .iter()
            .map(|tx_hash| {
                let tx = self.state.transactions().get(tx_hash).unwrap();
                (*tx_hash, tx.clone())
            })
            .collect::<Vec<_>>();
        let (block_hash, txs, patch) = self.create_block(propose.height(),
                                                         propose.round(),
                                                         propose.time(),
                                                         txs.as_slice());
        // Save patch
        self.state.add_block(block_hash, patch, txs, propose.round());
        block_hash
    }

    pub fn request_propose_or_txs(&mut self, propose_hash: &Hash, validator: ValidatorId) -> bool {
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

        if let Some(data) = requested_data.clone() {
            let key = self.public_key_of(validator);
            self.request(data, key);
            false
        } else {
            true
        }
    }

    pub fn request_next_block(&mut self) {
        // TODO randomize next peer
        let heights = self.state.validator_heights();
        if !heights.is_empty() {
            for id in heights {
                let peer = *self.state.public_key_of(id).unwrap();
                if self.state.peers().contains_key(&peer) {
                    let height = self.state.height();
                    self.request(RequestData::Block(height), peer);
                    break;
                }
            }
        }
    }

    pub fn remove_request(&mut self, data: RequestData) -> HashSet<PublicKey> {
        // TODO: clear timeout
        self.state.remove_request(&data)
    }

    pub fn broadcast_prevote(&mut self, round: Round, propose_hash: &Hash) -> bool {
        let locked_round = self.state.locked_round();
        let prevote = Prevote::new(self.state.id(),
                                   self.state.height(),
                                   round,
                                   propose_hash,
                                   locked_round,
                                   &self.secret_key);
        let has_majority_prevotes = self.state.add_prevote(&prevote);
        trace!("Broadcast prevote: {:?}", prevote);
        self.broadcast(prevote.raw());
        has_majority_prevotes
    }

    pub fn broadcast_precommit(&mut self, round: Round, propose_hash: &Hash, block_hash: &Hash) {
        let precommit = Precommit::new(self.state.id(),
                                       self.state.height(),
                                       round,
                                       propose_hash,
                                       block_hash,
                                       &self.secret_key);
        self.state.add_precommit(&precommit);
        trace!("Broadcast precommit: {:?}", precommit);
        self.broadcast(precommit.raw());
    }

    // TODO reuse where is possible
    pub fn verify_precommit(&self,
                            block_hash: &Hash,
                            block_height: Height,
                            precommit_round: Round,
                            precommit: &Precommit)
                            -> Result<(), String> {
        if let Some(pub_key) = self.state.public_key_of(precommit.validator()) {
            if !precommit.verify_signature(pub_key) {
                let e = format!("Received wrong signed precommit, precommit={:?}", precommit);
                return Err(e);
            }
            if precommit.block_hash() != block_hash {
                let e = format!("Received precommit with wrong block_hash, precommit={:?}",
                                precommit);
                return Err(e);
            }
            if precommit.height() != block_height {
                let e = format!("Received precommit with wrong height, precommit={:?}",
                                precommit);
                return Err(e);
            }
            if precommit.round() != precommit_round {
                let e = format!("Received precommits with the different rounds, precommit={:?}",
                                precommit);
                return Err(e);
            }
        } else {
            let e = format!("Received precommit with wrong validator, precommit={:?}",
                            precommit);
            return Err(e);
        }
        Ok(())
    }

    fn public_key_of(&self, id: ValidatorId) -> PublicKey {
        *self.state.public_key_of(id).unwrap()
    }
}
