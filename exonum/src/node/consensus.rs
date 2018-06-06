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

use std::{collections::HashSet, error::Error};

use blockchain::{Schema, Transaction};
use crypto::{CryptoHash, Hash, PublicKey};
use events::InternalRequest;
use helpers::{Height, Round, ValidatorId};
use messages::{BlockRequest, BlockResponse, ConsensusMessage, Message, Precommit, Prevote,
               PrevotesRequest, Propose, ProposeRequest, RawTransaction, TransactionsRequest,
               TransactionsResponse};
use node::{NodeHandler, RequestData};
use storage::Patch;

// TODO reduce view invocations (ECR-171)
impl NodeHandler {
    /// Validates consensus message, then redirects it to the corresponding `handle_...` function.
    #[cfg_attr(feature = "flame_profile", flame)]
    pub fn handle_consensus(&mut self, msg: ConsensusMessage) {
        if !self.is_enabled {
            info!(
                "Ignoring a consensus message {:?} because the node is disabled",
                msg
            );
            return;
        }

        // Ignore messages from previous and future height
        if msg.height() < self.state.height() || msg.height() > self.state.height().next() {
            warn!(
                "Received consensus message from other height: msg.height={}, self.height={}",
                msg.height(),
                self.state.height()
            );
            return;
        }

        // Queued messages from next height or round
        // TODO: should we ignore messages from far rounds (ECR-171)?
        if msg.height() == self.state.height().next() || msg.round() > self.state.round() {
            trace!(
                "Received consensus message from future round: msg.height={}, msg.round={}, \
                 self.height={}, self.round={}",
                msg.height(),
                msg.round(),
                self.state.height(),
                self.state.round()
            );
            let validator = msg.validator();
            let round = msg.round();
            self.state.add_queued(msg);
            trace!("Trying to reach actual round.");
            if let Some(r) = self.state.update_validator_round(validator, round) {
                trace!("Scheduling jump to round.");
                let height = self.state.height();
                self.execute_later(InternalRequest::JumpToRound(height, r));
            }
            return;
        }

        let key = match self.state.consensus_public_key_of(msg.validator()) {
            Some(public_key) => {
                if !msg.verify(&public_key) {
                    error!(
                        "Received consensus message with incorrect signature, msg={:?}",
                        msg
                    );
                    return;
                }
                public_key
            }
            None => {
                error!("Received message from incorrect validator, msg={:?}", msg);
                return;
            }
        };

        trace!("Handle message={:?}", msg);
        match msg {
            ConsensusMessage::Propose(msg) => self.handle_propose(key, &msg),
            ConsensusMessage::Prevote(msg) => self.handle_prevote(key, &msg),
            ConsensusMessage::Precommit(msg) => self.handle_precommit(key, &msg),
        }
    }

    /// Handles the `Propose` message. For details see the message documentation.
    pub fn handle_propose(&mut self, from: PublicKey, msg: &Propose) {
        debug_assert_eq!(
            Some(from),
            self.state.consensus_public_key_of(msg.validator())
        );

        // Check prev_hash
        if msg.prev_hash() != self.state.last_hash() {
            error!("Received propose with wrong last_block_hash msg={:?}", msg);
            return;
        }

        // Check leader
        if msg.validator() != self.state.leader(msg.round()) {
            error!(
                "Wrong propose leader detected: actual={}, expected={}",
                msg.validator(),
                self.state.leader(msg.round())
            );
            return;
        }

        trace!("Handle propose");

        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&*snapshot);
        //TODO: remove this match after errors refactor. (ECR-979)
        let has_unknown_txs =
            match self.state
                .add_propose(msg, &schema.transactions(), &schema.transactions_pool())
            {
                Ok(state) => state.has_unknown_txs(),
                Err(err) => {
                    warn!("{}, msg={:?}", err, msg);
                    return;
                }
            };

        let hash = msg.hash();

        // Remove request info
        let known_nodes = self.remove_request(&RequestData::Propose(hash));

        if has_unknown_txs {
            trace!("REQUEST TRANSACTIONS");
            self.request(RequestData::Transactions(hash), from);

            for node in known_nodes {
                self.request(RequestData::Transactions(hash), node);
            }
        } else {
            self.handle_full_propose(hash, msg.round());
        }
    }

    // Validates transaction from the block and appends them into the pool.
    // Returns tuple of transaction hashes and Patch of changes in the pool.
    fn validate_block_transactions(&self, block: &BlockResponse) -> Option<(Vec<Hash>, Patch)> {
        let mut fork = self.blockchain.fork();
        let mut tx_hashes = Vec::new();
        {
            let mut schema = Schema::new(&mut fork);
            for raw in block.transactions() {
                let tx = match self.blockchain.tx_from_raw(raw) {
                    Ok(tx) => tx,
                    Err(e) => {
                        error!("{}, block={:?}", e.description(), block);
                        return None;
                    }
                };

                let hash = tx.hash();
                if schema.transactions().contains(&hash)
                    && !schema.transactions_pool().contains(&hash)
                {
                    error!(
                        "Received block with already committed transaction, block={:?}",
                        block
                    );
                    return None;
                }
                profiler_span!("tx.verify()", {
                    if !tx.verify() {
                        error!("Incorrect transaction in block detected, block={:?}", block);
                        return None;
                    }
                });
                schema.add_transaction_into_pool(tx.raw().clone());
                tx_hashes.push(hash);
            }
        }
        Some((tx_hashes, fork.into_patch()))
    }

    /// Handles the `Block` message. For details see the message documentation.
    // TODO write helper function which returns Result (ECR-123)
    #[cfg_attr(feature = "flame_profile", flame)]
    pub fn handle_block(&mut self, msg: &BlockResponse) {
        // Request are sent to us
        if msg.to() != self.state.consensus_public_key() {
            error!(
                "Received block intended for another peer, to={}, from={}",
                msg.to().to_hex(),
                msg.from().to_hex()
            );
            return;
        }

        if !self.state.whitelist().allow(msg.from()) {
            error!(
                "Received request message from peer = {} which not in whitelist.",
                msg.from().to_hex()
            );
            return;
        }

        if !msg.verify_signature(msg.from()) {
            error!("Received block with incorrect signature, msg={:?}", msg);
            return;
        }

        trace!("Handle block");

        let block = msg.block();
        let block_hash = block.hash();

        // TODO add block with greater height to queue (ECR-171)
        if self.state.height() != block.height() {
            return;
        }

        // Check block content
        if block.prev_hash() != &self.last_block_hash() {
            error!(
                "Received block prev_hash is distinct from the one in db, \
                 block={:?}, block.prev_hash={:?}, db.last_block_hash={:?}",
                msg,
                *block.prev_hash(),
                self.last_block_hash()
            );
            return;
        }

        if let Err(err) = self.verify_precommits(&msg.precommits(), &block_hash, block.height()) {
            error!("{}, block={:?}", err, msg);
            return;
        }

        if self.state.block(&block_hash).is_none() {
            // Verify transactions
            let tx_hashes = if let Some(res) = self.validate_block_transactions(msg) {
                self.blockchain
                    .merge(res.1)
                    .expect("Unable to save transaction to persistent pool.");
                res.0
            } else {
                return;
            };

            let (block_hash, patch) =
                self.create_block(block.proposer_id(), block.height(), tx_hashes.as_slice());
            // Verify block_hash
            if block_hash != block.hash() {
                panic!(
                    "Block_hash incorrect in the received block={:?}. Either a node's \
                     implementation is incorrect or validators majority works incorrectly",
                    msg
                );
            }

            // Commit block
            self.state
                .add_block(block_hash, patch, tx_hashes, block.proposer_id());
        }
        self.commit(block_hash, msg.precommits().iter(), None);
        self.request_next_block();
    }

    /// Executes and commits block. This function is called when node has full propose information.
    pub fn handle_full_propose(&mut self, hash: Hash, propose_round: Round) {
        // Send prevote
        if self.state.locked_round() == Round::zero() {
            if self.state.is_validator() && !self.state.have_prevote(propose_round) {
                self.broadcast_prevote(propose_round, &hash);
            } else {
                // TODO: what if we HAVE prevote for the propose round (ECR-171)?
            }
        }

        // Lock to propose
        // TODO: avoid loop here (ECR-171).
        let start_round = ::std::cmp::max(self.state.locked_round().next(), propose_round);
        for round in start_round.iter_to(self.state.round().next()) {
            if self.state.has_majority_prevotes(round, hash) {
                self.handle_majority_prevotes(round, &hash);
            }
        }

        // Commit propose
        for (round, block_hash) in self.state.unknown_propose_with_precommits(&hash) {
            // Execute block and get state hash
            let our_block_hash = self.execute(&hash);

            if our_block_hash != block_hash {
                panic!(
                    "Full propose: wrong state hash. Either a node's implementation is \
                     incorrect or validators majority works incorrectly"
                );
            }

            let precommits = self.state.precommits(round, our_block_hash).to_vec();
            self.commit(our_block_hash, precommits.iter(), Some(propose_round));
        }
    }

    /// Handles the `Prevote` message. For details see the message documentation.
    pub fn handle_prevote(&mut self, from: PublicKey, msg: &Prevote) {
        trace!("Handle prevote");

        debug_assert_eq!(
            Some(from),
            self.state.consensus_public_key_of(msg.validator())
        );

        // Add prevote
        let has_consensus = self.state.add_prevote(msg);

        // Request propose or transactions
        let has_propose_with_txs = self.request_propose_or_txs(msg.propose_hash(), from);

        // Request prevotes
        if msg.locked_round() > self.state.locked_round() {
            self.request(
                RequestData::Prevotes(msg.locked_round(), *msg.propose_hash()),
                from,
            );
        }

        // Lock to propose
        if has_consensus && has_propose_with_txs {
            self.handle_majority_prevotes(msg.round(), msg.propose_hash());
        }
    }

    /// Locks to the propose by calling `lock`. This function is called when node receives
    /// +2/3 pre-votes.
    pub fn handle_majority_prevotes(&mut self, prevote_round: Round, propose_hash: &Hash) {
        // Remove request info
        self.remove_request(&RequestData::Prevotes(prevote_round, *propose_hash));
        // Lock to propose
        if self.state.locked_round() < prevote_round && self.state.propose(propose_hash).is_some() {
            self.lock(prevote_round, *propose_hash);
        }
    }

    /// Executes and commits block. This function is called when the node has +2/3 pre-commits.
    pub fn handle_majority_precommits(
        &mut self,
        round: Round,
        propose_hash: &Hash,
        block_hash: &Hash,
    ) {
        // Check if propose is known.
        if self.state.propose(propose_hash).is_none() {
            self.state
                .add_unknown_propose_with_precommits(round, *propose_hash, *block_hash);
            return;
        }

        // Request transactions if needed.
        let proposer = {
            let propose_state = self.state.propose(propose_hash).unwrap();
            if propose_state.has_unknown_txs() {
                Some(
                    self.state
                        .consensus_public_key_of(propose_state.message().validator())
                        .unwrap(),
                )
            } else {
                None
            }
        };
        if let Some(proposer) = proposer {
            self.request(RequestData::Transactions(*propose_hash), proposer);
            return;
        }

        // Execute block and get state hash
        let our_block_hash = self.execute(propose_hash);
        assert_eq!(
            &our_block_hash, block_hash,
            "Our block_hash different from precommits one."
        );

        // Commit.
        let precommits = self.state.precommits(round, our_block_hash).to_vec();
        self.commit(our_block_hash, precommits.iter(), Some(round));
    }

    /// Locks node to the specified round, so pre-votes for the lower round will be ignored.
    pub fn lock(&mut self, prevote_round: Round, propose_hash: Hash) {
        trace!("MAKE LOCK {:?} {:?}", prevote_round, propose_hash);
        for round in prevote_round.iter_to(self.state.round().next()) {
            // Send prevotes
            if self.state.is_validator() && !self.state.have_prevote(round) {
                self.broadcast_prevote(round, &propose_hash);
            }

            // Change lock
            if self.state.has_majority_prevotes(round, propose_hash) {
                // Put consensus messages for current Propose and this round to the cache.
                self.check_propose_saved(round, &propose_hash);
                let raw_messages = self.state
                    .prevotes(prevote_round, propose_hash)
                    .iter()
                    .map(|msg| msg.raw().clone())
                    .collect::<Vec<_>>();
                self.blockchain.save_messages(round, raw_messages);

                self.state.lock(round, propose_hash);
                // Send precommit
                if self.state.is_validator() && !self.state.have_incompatible_prevotes() {
                    // Execute block and get state hash
                    let block_hash = self.execute(&propose_hash);
                    self.broadcast_precommit(round, &propose_hash, &block_hash);
                    // Commit if has consensus
                    if self.state.has_majority_precommits(round, block_hash) {
                        self.handle_majority_precommits(round, &propose_hash, &block_hash);
                        return;
                    }
                }
                // Remove request info
                self.remove_request(&RequestData::Prevotes(round, propose_hash));
            }
        }
    }

    /// Handles the `Precommit` message. For details see the message documentation.
    pub fn handle_precommit(&mut self, from: PublicKey, msg: &Precommit) {
        trace!("Handle precommit");

        debug_assert_eq!(
            Some(from),
            self.state.consensus_public_key_of(msg.validator())
        );

        // Add precommit
        let has_consensus = self.state.add_precommit(msg);

        // Request propose
        if self.state.propose(msg.propose_hash()).is_none() {
            self.request(RequestData::Propose(*msg.propose_hash()), from);
        }

        // Request prevotes
        // TODO: If Precommit sender in on a greater height, then it cannot have +2/3 prevotes.
        // So can we get rid of useless sending RequestPrevotes message (ECR-171)?
        if msg.round() > self.state.locked_round() {
            self.request(
                RequestData::Prevotes(msg.round(), *msg.propose_hash()),
                from,
            );
        }

        // Has majority precommits
        if has_consensus {
            self.handle_majority_precommits(msg.round(), msg.propose_hash(), msg.block_hash());
        }
    }

    /// Commits block, so new height is achieved.
    // FIXME: push precommits into storage
    pub fn commit<'a, I: Iterator<Item = &'a Precommit>>(
        &mut self,
        block_hash: Hash,
        precommits: I,
        round: Option<Round>,
    ) {
        trace!("COMMIT {:?}", block_hash);

        // Merge changes into storage
        let (committed_txs, proposer) = {
            // FIXME Avoid of clone here.
            let block_state = self.state.block(&block_hash).unwrap().clone();
            self.blockchain
                .commit(block_state.patch(), block_hash, precommits)
                .unwrap();
            // Update node state
            self.state
                .update_config(Schema::new(&self.blockchain.snapshot()).actual_configuration());
            // Update state to new height
            let block_hash = self.blockchain.last_hash();
            self.state
                .new_height(&block_hash, self.system_state.current_time());
            (block_state.txs().len(), block_state.proposer_id())
        };
        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        let pool_len = schema.transactions_pool_len();

        metric!("node.mempool", pool_len);

        let height = self.state.height();
        info!(
            "COMMIT ====== height={}, proposer={}, round={}, committed={}, pool={}, hash={}",
            height,
            proposer,
            round
                .map(|x| format!("{}", x))
                .unwrap_or_else(|| "?".into()),
            committed_txs,
            pool_len,
            block_hash.to_hex(),
        );

        // TODO: reset status timeout (ECR-171).
        self.broadcast_status();
        self.add_status_timeout();

        // Add timeout for first round
        self.add_round_timeout();
        // Send propose we is leader
        if self.state.is_leader() {
            self.add_propose_timeout();
        }

        // Handle queued messages
        for msg in self.state.queued() {
            self.handle_consensus(msg);
        }
    }

    /// Checks if the transaction is new and adds it to the pool.
    fn handle_tx_inner(&mut self, msg: RawTransaction) -> Result<(), String> {
        let hash = msg.hash();

        profiler_span!("Make sure that it is new transaction", {
            let snapshot = self.blockchain.snapshot();
            if Schema::new(&snapshot).transactions().contains(&hash) {
                let err = format!("Received already processed transaction, hash {:?}", hash);
                return Err(err);
            }
        });

        let mut fork = self.blockchain.fork();
        {
            let mut schema = Schema::new(&mut fork);
            schema.add_transaction_into_pool(msg);
        }
        self.blockchain
            .merge(fork.into_patch())
            .expect("Unable to save transaction to persistent pool.");

        let full_proposes = self.state.check_incomplete_proposes(hash);
        // Go to handle full propose if we get last transaction
        for (hash, round) in full_proposes {
            self.remove_request(&RequestData::Transactions(hash));
            self.handle_full_propose(hash, round);
        }
        Ok(())
    }

    /// Handles raw transaction. Transaction is ignored if it is already known, otherwise it is
    /// added to the transactions pool.
    #[cfg_attr(feature = "flame_profile", flame)]
    pub fn handle_tx(&mut self, msg: RawTransaction) {
        let tx = match self.blockchain.tx_from_raw(msg.clone()) {
            Ok(tx) => tx,
            Err(e) => {
                let service_id = msg.service_id();
                error!("{}, service_id={}", e.description(), service_id);
                return;
            }
        };

        profiler_span!("tx.verify()", {
            if !tx.verify() {
                return;
            }
        });

        // We don't care about result, because situation when transaction received twice
        // is normal for internal messages (transaction may be received from 2+ nodes).
        let _ = self.handle_tx_inner(msg);
    }

    /// Handles raw transactions.
    #[cfg_attr(feature = "flame_profile", flame)]
    pub fn handle_txs_batch(&mut self, msg: &TransactionsResponse) {
        if msg.to() != self.state.consensus_public_key() {
            error!(
                "Received response intended for another peer, to={}, from={}",
                msg.to().to_hex(),
                msg.from().to_hex()
            );
            return;
        }

        if !self.state.whitelist().allow(msg.from()) {
            error!(
                "Received response message from peer = {} which not in whitelist.",
                msg.from().to_hex()
            );
            return;
        }

        if !msg.verify_signature(msg.from()) {
            error!("Received response with incorrect signature, msg={:?}", msg);
            return;
        }

        for tx in msg.transactions() {
            self.handle_tx(tx);
        }
    }

    /// Handles external boxed transaction. Additionally transaction will be broadcast to the
    /// Node's peers.
    #[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
    pub fn handle_incoming_tx(&mut self, msg: Box<Transaction>) {
        trace!("Handle incoming transaction");
        match self.handle_tx_inner(msg.raw().clone()) {
            Ok(_) => self.broadcast(msg.raw()),
            Err(e) => error!("{}", e),
        }
    }

    /// Handle new round, after jump.
    pub fn handle_new_round(&mut self, height: Height, round: Round) {
        trace!("Handle new round");
        if height != self.state.height() {
            return;
        }

        if round <= self.state.round() {
            return;
        }

        info!("Jump to a new round = {}", round);
        self.state.jump_round(round);
        self.add_round_timeout();
        self.process_new_round();
    }

    // Try to process consensus messages from the future round.
    fn process_new_round(&mut self) {
        if self.state.is_validator() {
            // Send prevote if we are locked or propose if we are leader
            if let Some(hash) = self.state.locked_propose() {
                let round = self.state.round();
                let has_majority_prevotes = self.broadcast_prevote(round, &hash);
                if has_majority_prevotes {
                    self.handle_majority_prevotes(round, &hash);
                }
            } else if self.state.is_leader() {
                self.add_propose_timeout();
            }
        }

        // Handle queued messages
        for msg in self.state.queued() {
            self.handle_consensus(msg);
        }
    }
    /// Handles round timeout. As result node sends `Propose` if it is a leader or `Prevote` if it
    /// is locked to some round.
    pub fn handle_round_timeout(&mut self, height: Height, round: Round) {
        // TODO debug asserts (ECR-171)?
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

        self.process_new_round();
    }

    /// Handles propose timeout. Node sends `Propose` and `Prevote` if it is a leader as result.
    pub fn handle_propose_timeout(&mut self, height: Height, round: Round) {
        // TODO debug asserts (ECR-171)?
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
        if let Some(validator_id) = self.state.validator_id() {
            if self.state.have_prevote(round) {
                return;
            }
            let snapshot = self.blockchain.snapshot();
            let schema = Schema::new(&snapshot);
            let pool = schema.transactions_pool();
            let pool_len = schema.transactions_pool_len();

            info!("LEADER: pool = {}", pool_len);

            let round = self.state.round();
            let max_count = ::std::cmp::min(self.txs_block_limit() as usize, pool_len);

            let txs: Vec<Hash> = pool.iter().take(max_count).collect();
            let propose = Propose::new(
                validator_id,
                self.state.height(),
                round,
                self.state.last_hash(),
                &txs,
                self.state.consensus_secret_key(),
            );

            // Put our propose to the consensus messages cache
            self.blockchain.save_message(round, propose.raw());

            trace!("Broadcast propose: {:?}", propose);
            self.broadcast(propose.raw());

            // Save our propose into state
            let hash = self.state.add_self_propose(propose);

            // Send prevote
            let has_majority_prevotes = self.broadcast_prevote(round, &hash);
            if has_majority_prevotes {
                self.handle_majority_prevotes(round, &hash);
            }
        }
    }

    /// Handles request timeout by sending the corresponding request message to a peer.
    pub fn handle_request_timeout(&mut self, data: &RequestData, peer: Option<PublicKey>) {
        trace!("HANDLE REQUEST TIMEOUT");
        // FIXME: check height?
        if let Some(peer) = self.state.retry(data, peer) {
            self.add_request_timeout(data.clone(), Some(peer));

            let message = match *data {
                RequestData::Propose(ref propose_hash) => ProposeRequest::new(
                    self.state.consensus_public_key(),
                    &peer,
                    self.state.height(),
                    propose_hash,
                    self.state.consensus_secret_key(),
                ).raw()
                    .clone(),
                RequestData::Transactions(ref propose_hash) => {
                    let txs: Vec<_> = self.state
                        .propose(propose_hash)
                        .unwrap()
                        .unknown_txs()
                        .iter()
                        .cloned()
                        .collect();
                    TransactionsRequest::new(
                        self.state.consensus_public_key(),
                        &peer,
                        &txs,
                        self.state.consensus_secret_key(),
                    ).raw()
                        .clone()
                }
                RequestData::Prevotes(round, ref propose_hash) => PrevotesRequest::new(
                    self.state.consensus_public_key(),
                    &peer,
                    self.state.height(),
                    round,
                    propose_hash,
                    self.state.known_prevotes(round, propose_hash),
                    self.state.consensus_secret_key(),
                ).raw()
                    .clone(),
                RequestData::Block(height) => BlockRequest::new(
                    self.state.consensus_public_key(),
                    &peer,
                    height,
                    self.state.consensus_secret_key(),
                ).raw()
                    .clone(),
            };
            trace!("Send request {:?} to peer {:?}", data, peer);
            self.send_to_peer(peer, &message);
        }
    }

    /// Creates block with given transaction and returns its hash and corresponding changes.
    pub fn create_block(
        &mut self,
        proposer_id: ValidatorId,
        height: Height,
        tx_hashes: &[Hash],
    ) -> (Hash, Patch) {
        self.blockchain.create_patch(proposer_id, height, tx_hashes)
    }

    /// Calls `create_block` with transactions from the corresponding `Propose` and returns the
    /// block hash.
    #[cfg_attr(feature = "flame_profile", flame)]
    pub fn execute(&mut self, propose_hash: &Hash) -> Hash {
        // if we already execute this block, return hash
        if let Some(hash) = self.state.propose_mut(propose_hash).unwrap().block_hash() {
            return hash;
        }
        let propose = self.state.propose(propose_hash).unwrap().message().clone();

        let tx_hashes = propose.transactions().to_vec();

        let (block_hash, patch) =
            self.create_block(propose.validator(), propose.height(), tx_hashes.as_slice());
        // Save patch
        self.state
            .add_block(block_hash, patch, tx_hashes, propose.validator());
        self.state
            .propose_mut(propose_hash)
            .unwrap()
            .set_block_hash(block_hash);
        block_hash
    }

    /// Returns `true` if propose and all transactions are known, otherwise requests needed data
    /// and returns `false`.
    pub fn request_propose_or_txs(&mut self, propose_hash: &Hash, key: PublicKey) -> bool {
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
            self.request(data, key);
            false
        } else {
            true
        }
    }

    /// Requests a block for the next height from all peers with a bigger height. Called when the
    /// node tries to catch up with other nodes' height.
    pub fn request_next_block(&mut self) {
        // TODO randomize next peer (ECR-171)
        let heights: Vec<_> = self.state
            .nodes_with_bigger_height()
            .into_iter()
            .cloned()
            .collect();
        if !heights.is_empty() {
            for peer in heights {
                if self.state.peers().contains_key(&peer) {
                    let height = self.state.height();
                    self.request(RequestData::Block(height), peer);
                    break;
                }
            }
        }
    }

    /// Removes the specified request from the pending request list.
    pub fn remove_request(&mut self, data: &RequestData) -> HashSet<PublicKey> {
        // TODO: clear timeout (ECR-171)
        self.state.remove_request(data)
    }

    /// Broadcasts the `Prevote` message to all peers.
    pub fn broadcast_prevote(&mut self, round: Round, propose_hash: &Hash) -> bool {
        let validator_id = self.state
            .validator_id()
            .expect("called broadcast_prevote in Auditor node.");
        let locked_round = self.state.locked_round();
        let prevote = Prevote::new(
            validator_id,
            self.state.height(),
            round,
            propose_hash,
            locked_round,
            self.state.consensus_secret_key(),
        );
        let has_majority_prevotes = self.state.add_prevote(&prevote);

        // save outgoing Prevote to the consensus messages cache before broadcast
        self.check_propose_saved(round, propose_hash);
        self.blockchain.save_message(round, prevote.raw());

        trace!("Broadcast prevote: {:?}", prevote);
        self.broadcast(prevote.raw());

        has_majority_prevotes
    }

    /// Broadcasts the `Precommit` message to all peers.
    pub fn broadcast_precommit(&mut self, round: Round, propose_hash: &Hash, block_hash: &Hash) {
        let validator_id = self.state
            .validator_id()
            .expect("called broadcast_precommit in Auditor node.");
        let precommit = Precommit::new(
            validator_id,
            self.state.height(),
            round,
            propose_hash,
            block_hash,
            self.system_state.current_time().into(),
            self.state.consensus_secret_key(),
        );
        self.state.add_precommit(&precommit);

        // Put our Precommit to the consensus cache before broadcast
        self.blockchain.save_message(round, precommit.raw());

        trace!("Broadcast precommit: {:?}", precommit);
        self.broadcast(precommit.raw());
    }

    /// Checks that pre-commits count is correct and calls `verify_precommit` for each of them.
    fn verify_precommits(
        &self,
        precommits: &[Precommit],
        block_hash: &Hash,
        block_height: Height,
    ) -> Result<(), String> {
        if precommits.len() < self.state.majority_count() {
            return Err("Received block without consensus".to_string());
        } else if precommits.len() > self.state.validators().len() {
            return Err("Wrong precommits count in block".to_string());
        }

        let mut validators = HashSet::with_capacity(precommits.len());
        let round = precommits[0].round();
        for precommit in precommits {
            if !validators.insert(precommit.validator()) {
                return Err("Several precommits from one validator in block".to_string());
            }

            self.verify_precommit(block_hash, block_height, round, precommit)?;
        }

        Ok(())
    }

    /// Verifies that `Precommit` contains correct block hash, height round and is signed by the
    /// right validator.
    fn verify_precommit(
        &self,
        block_hash: &Hash,
        block_height: Height,
        precommit_round: Round,
        precommit: &Precommit,
    ) -> Result<(), String> {
        if let Some(pub_key) = self.state.consensus_public_key_of(precommit.validator()) {
            if !precommit.verify_signature(&pub_key) {
                let e = format!("Received wrong signed precommit, precommit={:?}", precommit);
                return Err(e);
            }
            if precommit.block_hash() != block_hash {
                let e = format!(
                    "Received precommit with wrong block_hash, precommit={:?}",
                    precommit
                );
                return Err(e);
            }
            if precommit.height() != block_height {
                let e = format!(
                    "Received precommit with wrong height, precommit={:?}",
                    precommit
                );
                return Err(e);
            }
            if precommit.round() != precommit_round {
                let e = format!(
                    "Received precommits with the different rounds, precommit={:?}",
                    precommit
                );
                return Err(e);
            }
        } else {
            let e = format!(
                "Received precommit with wrong validator, precommit={:?}",
                precommit
            );
            return Err(e);
        }
        Ok(())
    }

    /// Checks whether Propose is saved to the consensus cache and saves it otherwise
    fn check_propose_saved(&mut self, round: Round, propose_hash: &Hash) {
        if let Some(propose_state) = self.state.propose_mut(propose_hash) {
            if !propose_state.is_saved() {
                self.blockchain
                    .save_message(round, propose_state.message().raw());
                propose_state.set_saved(true);
            }
        }
    }
}
