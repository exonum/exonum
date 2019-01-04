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

use std::collections::HashSet;

use blockchain::Schema;
use crypto::{CryptoHash, Hash, PublicKey};
use events::InternalRequest;
use failure;
use helpers::{Height, Round, ValidatorId};
use messages::{
    BlockRequest, BlockResponse, Consensus as ConsensusMessage, Precommit, Prevote,
    PrevotesRequest, Propose, ProposeRequest, RawTransaction, Signed, SignedMessage,
    TransactionsRequest, TransactionsResponse,
};
use node::{NodeHandler, RequestData};
use storage::Patch;

// TODO Reduce view invocations. (ECR-171)
impl NodeHandler {
    /// Validates consensus message, then redirects it to the corresponding `handle_...` function.
    pub fn handle_consensus(&mut self, msg: ConsensusMessage) {
        if !self.is_enabled {
            info!(
                "Ignoring a consensus message {:?} because the node is disabled",
                msg
            );
            return;
        }

        // Warning for messages from previous and future height
        if msg.height() < self.state.height().previous()
            || msg.height() > self.state.height().next()
        {
            warn!(
                "Received consensus message from other height: msg.height={}, self.height={}",
                msg.height(),
                self.state.height()
            );
        }

        // Ignore messages from previous and future height
        if msg.height() < self.state.height() || msg.height() > self.state.height().next() {
            return;
        }

        // Queued messages from next height or round
        // TODO: Should we ignore messages from far rounds? (ECR-171)
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
        let key = msg.author();

        trace!("Handle message={:?}", msg);

        match msg {
            ConsensusMessage::Propose(ref msg) => self.handle_propose(key, msg),
            ConsensusMessage::Prevote(ref msg) => self.handle_prevote(key, msg),
            ConsensusMessage::Precommit(ref msg) => self.handle_precommit(key, msg),
        }
    }

    /// Handles the `Propose` message. For details see the message documentation.
    pub fn handle_propose(&mut self, from: PublicKey, msg: &Signed<Propose>) {
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
        let schema = Schema::new(snapshot);
        //TODO: Remove this match after errors refactor. (ECR-979)
        let has_unknown_txs = match self.state.add_propose(
            msg.clone(),
            &schema.transactions(),
            &schema.transactions_pool(),
        ) {
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
            self.request(RequestData::ProposeTransactions(hash), from);

            for node in known_nodes {
                self.request(RequestData::ProposeTransactions(hash), node);
            }
        } else {
            self.handle_full_propose(hash, msg.round());
        }
    }

    fn validate_block_response(&self, msg: &Signed<BlockResponse>) -> Result<(), failure::Error> {
        if msg.to() != self.state.consensus_public_key() {
            bail!(
                "Received block intended for another peer, to={}, from={}",
                msg.to().to_hex(),
                msg.author().to_hex()
            );
        }

        if !self.state.connect_list().is_peer_allowed(&msg.author()) {
            bail!(
                "Received request message from peer = {} which not in ConnectList.",
                msg.author().to_hex()
            );
        }

        let block = msg.block();
        let block_hash = block.hash();

        // TODO: Add block with greater height to queue. (ECR-171)
        if self.state.height() != block.height() {
            bail!("Received block has another height, msg={:?}", msg);
        }

        // Check block content.
        if block.prev_hash() != &self.last_block_hash() {
            bail!(
                "Received block prev_hash is distinct from the one in db, \
                 block={:?}, block.prev_hash={:?}, db.last_block_hash={:?}",
                msg,
                *block.prev_hash(),
                self.last_block_hash()
            );
        }

        if self.state.incomplete_block().is_some() {
            bail!("Already there is an incomplete block, msg={:?}", msg);
        }

        if !msg.verify_tx_hash() {
            bail!("Received block has invalid tx_hash, msg={:?}", msg);
        }
        let precommits: Result<Vec<_>, _> = msg
            .precommits()
            .into_iter()
            .map(Precommit::verify_precommit)
            .collect();
        self.verify_precommits(&precommits?, &block_hash, block.height())?;

        Ok(())
    }

    /// Handles the `Block` message. For details see the message documentation.
    // TODO: Write helper function which returns Result. (ECR-123)
    pub fn handle_block(&mut self, msg: &Signed<BlockResponse>) -> Result<(), failure::Error> {
        self.validate_block_response(&msg)?;

        let block = msg.block();
        let block_hash = block.hash();
        if self.state.block(&block_hash).is_none() {
            let snapshot = self.blockchain.snapshot();
            let schema = Schema::new(snapshot);
            let has_unknown_txs = self
                .state
                .create_incomplete_block(&msg, &schema.transactions(), &schema.transactions_pool())
                .has_unknown_txs();

            let known_nodes = self.remove_request(&RequestData::Block(block.height()));

            if has_unknown_txs {
                trace!("REQUEST TRANSACTIONS");
                self.request(RequestData::BlockTransactions, msg.author());

                for node in known_nodes {
                    self.request(RequestData::BlockTransactions, node);
                }
            } else {
                self.handle_full_block(&msg)?;
            }
        } else {
            let precommits: Result<Vec<_>, _> = msg
                .precommits()
                .into_iter()
                .map(Precommit::verify_precommit)
                .collect();

            self.commit(block_hash, precommits?.into_iter(), None);
            self.request_next_block();
        }
        Ok(())
    }

    /// Executes and commits block. This function is called when node has full propose information.
    pub fn handle_full_propose(&mut self, hash: Hash, propose_round: Round) {
        // Send prevote
        if self.state.locked_round() == Round::zero() {
            if self.state.is_validator() && !self.state.have_prevote(propose_round) {
                self.broadcast_prevote(propose_round, &hash);
            } else {
                // TODO: what if we HAVE prevote for the propose round? (ECR-171)
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
        for (round, block_hash) in self.state.take_unknown_propose_with_precommits(&hash) {
            // Execute block and get state hash
            let our_block_hash = self.execute(&hash);

            if our_block_hash != block_hash {
                panic!(
                    "Full propose: wrong state hash. Either a node's implementation is \
                     incorrect or validators majority works incorrectly"
                );
            }

            let precommits = self.state.precommits(round, our_block_hash).to_vec();
            self.commit(our_block_hash, precommits.into_iter(), Some(propose_round));
        }
    }

    /// Executes and commits block. This function is called when node has full block information.
    ///
    /// # Panics
    ///
    /// Panics if the received block has incorrect `block_hash`.
    pub fn handle_full_block(&mut self, msg: &Signed<BlockResponse>) -> Result<(), failure::Error> {
        let block = msg.block();
        let block_hash = block.hash();

        if self.state.block(&block_hash).is_none() {
            let (computed_block_hash, patch) =
                self.create_block(block.proposer_id(), block.height(), msg.transactions());
            // Verify block_hash.
            assert!(
                computed_block_hash == block_hash,
                "Block_hash incorrect in the received block={:?}. Either a node's \
                 implementation is incorrect or validators majority works incorrectly",
                msg
            );

            self.state.add_block(
                computed_block_hash,
                patch,
                msg.transactions().to_vec(),
                block.proposer_id(),
            );
        }
        let precommits: Result<Vec<_>, _> = msg
            .precommits()
            .into_iter()
            .map(Precommit::verify_precommit)
            .collect();

        self.commit(block_hash, precommits?.into_iter(), None);
        self.request_next_block();
        Ok(())
    }

    /// Handles the `Prevote` message. For details see the message documentation.
    pub fn handle_prevote(&mut self, from: PublicKey, msg: &Signed<Prevote>) {
        trace!("Handle prevote");

        debug_assert_eq!(
            Some(from),
            self.state.consensus_public_key_of(msg.validator())
        );

        // Add prevote
        let has_consensus = self.state.add_prevote(msg.clone());

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
            self.request(RequestData::ProposeTransactions(*propose_hash), proposer);
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
        self.commit(our_block_hash, precommits.into_iter(), Some(round));
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
                let raw_messages = self
                    .state
                    .prevotes(prevote_round, propose_hash)
                    .iter()
                    .map(|p| p.clone().into())
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
    pub fn handle_precommit(&mut self, from: PublicKey, msg: &Signed<Precommit>) {
        trace!("Handle precommit");

        debug_assert_eq!(
            Some(from),
            self.state.consensus_public_key_of(msg.validator())
        );

        // Add precommit
        let has_consensus = self.state.add_precommit(msg.clone());

        // Request propose
        if self.state.propose(msg.propose_hash()).is_none() {
            self.request(RequestData::Propose(*msg.propose_hash()), from);
        }

        // Request prevotes
        // TODO: If Precommit sender in on a greater height, then it cannot have +2/3 prevotes.
        // So can we get rid of useless sending RequestPrevotes message? (ECR-171)
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
    pub fn commit<I: Iterator<Item = Signed<Precommit>>>(
        &mut self,
        block_hash: Hash,
        precommits: I,
        round: Option<Round>,
    ) {
        trace!("COMMIT {:?}", block_hash);

        // Merge changes into storage
        let (committed_txs, proposer) = {
            // FIXME: Avoid of clone here. (ECR-171)
            let block_state = self.state.block(&block_hash).unwrap().clone();
            self.blockchain
                .commit(block_state.patch(), block_hash, precommits)
                .unwrap();
            // Update node state.
            self.state
                .update_config(Schema::new(&self.blockchain.snapshot()).actual_configuration());
            // Update state to new height.
            let block_hash = self.blockchain.last_hash();
            self.state
                .new_height(&block_hash, self.system_state.current_time());
            (block_state.txs().len(), block_state.proposer_id())
        };

        self.api_state.broadcast(&block_hash);

        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        let pool_len = schema.transactions_pool_len();

        metric!("node.mempool", pool_len);

        let height = self.state.height();
        info!(
            "COMMIT ====== height={}, proposer={}, round={}, committed={}, pool={}, hash={}",
            height,
            proposer,
            round.map_or_else(|| "?".into(), |x| format!("{}", x)),
            committed_txs,
            pool_len,
            block_hash.to_hex(),
        );

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

    /// Checks if the transaction is new and adds it to the pool. This may trigger an expedited
    /// `Propose` timeout on this node if transaction count in the pool goes over the threshold.
    pub fn handle_tx(&mut self, msg: Signed<RawTransaction>) -> Result<(), failure::Error> {
        let hash = msg.hash();

        let snapshot = self.blockchain.snapshot();
        if Schema::new(&snapshot).transactions().contains(&hash) {
            bail!("Received already processed transaction, hash {:?}", hash)
        }

        if let Err(e) = self.blockchain.tx_from_raw(msg.payload().clone()) {
            error!("Received invalid transaction {:?}, result: {}", msg, e);
            bail!("Received malicious transaction.")
        }

        let mut fork = self.blockchain.fork();
        {
            let mut schema = Schema::new(&mut fork);
            schema.add_transaction_into_pool(msg);
        }
        self.blockchain
            .merge(fork.into_patch())
            .expect("Unable to save transaction to persistent pool.");

        if self.state.is_leader() && self.state.round() != Round::zero() {
            self.maybe_add_propose_timeout();
        }

        let full_proposes = self.state.check_incomplete_proposes(hash);
        // Go to handle full propose if we get last transaction.
        for (hash, round) in full_proposes {
            self.remove_request(&RequestData::ProposeTransactions(hash));
            self.handle_full_propose(hash, round);
        }

        let full_block = self.state.remove_unknown_transaction(hash);
        // Go to handle full block if we get last transaction
        if let Some(block) = full_block {
            self.remove_request(&RequestData::BlockTransactions);
            self.handle_full_block(block.message())?;
        }
        Ok(())
    }

    /// Handles raw transactions.
    pub fn handle_txs_batch(
        &mut self,
        msg: &Signed<TransactionsResponse>,
    ) -> Result<(), failure::Error> {
        if msg.to() != self.state.consensus_public_key() {
            bail!(
                "Received response intended for another peer, to={}, from={}",
                msg.to().to_hex(),
                msg.author().to_hex()
            )
        }

        if !self.state.connect_list().is_peer_allowed(&msg.author()) {
            bail!(
                "Received response message from peer = {} which not in ConnectList.",
                msg.author().to_hex()
            )
        }
        for tx in msg.transactions() {
            self.execute_later(InternalRequest::VerifyMessage(tx));
        }
        Ok(())
    }

    /// Handles external boxed transaction. Additionally transaction will be broadcast to the
    /// Node's peers.
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]
    pub fn handle_incoming_tx(&mut self, msg: Signed<RawTransaction>) {
        trace!("Handle incoming transaction");
        match self.handle_tx(msg.clone()) {
            Ok(_) => self.broadcast(msg),
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
        // TODO: Debug asserts? (ECR-171)
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
            let max_count = ::std::cmp::min(u64::from(self.txs_block_limit()), pool_len);

            let txs: Vec<Hash> = pool.iter().take(max_count as usize).collect();
            let propose = self.sign_message(Propose::new(
                validator_id,
                self.state.height(),
                round,
                self.state.last_hash(),
                &txs,
            ));
            // Put our propose to the consensus messages cache
            self.blockchain.save_message(round, propose.clone());

            trace!("Broadcast propose: {:?}", propose);
            self.broadcast(propose.clone());

            self.allow_expedited_propose = true;

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
        // FIXME: Check height? (ECR-171)
        if let Some(peer) = self.state.retry(data, peer) {
            self.add_request_timeout(data.clone(), Some(peer));

            let message: SignedMessage = match *data {
                RequestData::Propose(ref propose_hash) => self
                    .sign_message(ProposeRequest::new(
                        &peer,
                        self.state.height(),
                        propose_hash,
                    ))
                    .into(),
                RequestData::ProposeTransactions(ref propose_hash) => {
                    let txs: Vec<_> = self
                        .state
                        .propose(propose_hash)
                        .unwrap()
                        .unknown_txs()
                        .iter()
                        .cloned()
                        .collect();
                    self.sign_message(TransactionsRequest::new(&peer, &txs))
                        .into()
                }
                RequestData::BlockTransactions => {
                    let txs: Vec<_> = match self.state.incomplete_block() {
                        Some(incomplete_block) => {
                            incomplete_block.unknown_txs().iter().cloned().collect()
                        }
                        None => return,
                    };
                    self.sign_message(TransactionsRequest::new(&peer, &txs))
                        .into()
                }
                RequestData::Prevotes(round, ref propose_hash) => self
                    .sign_message(PrevotesRequest::new(
                        &peer,
                        self.state.height(),
                        round,
                        propose_hash,
                        self.state.known_prevotes(round, propose_hash),
                    ))
                    .into(),
                RequestData::Block(height) => {
                    self.sign_message(BlockRequest::new(&peer, height)).into()
                }
            };
            trace!("Send request {:?} to peer {:?}", data, peer);
            self.send_to_peer(peer, message);
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
                    Some(RequestData::ProposeTransactions(*propose_hash))
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
        // TODO: Randomize next peer. (ECR-171)
        let heights: Vec<_> = self
            .state
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
        // TODO: Clear timeout. (ECR-171)
        self.state.remove_request(data)
    }

    /// Broadcasts the `Prevote` message to all peers.
    pub fn broadcast_prevote(&mut self, round: Round, propose_hash: &Hash) -> bool {
        let validator_id = self
            .state
            .validator_id()
            .expect("called broadcast_prevote in Auditor node.");
        let locked_round = self.state.locked_round();
        let prevote = self.sign_message(Prevote::new(
            validator_id,
            self.state.height(),
            round,
            propose_hash,
            locked_round,
        ));
        let has_majority_prevotes = self.state.add_prevote(prevote.clone());

        // save outgoing Prevote to the consensus messages cache before broadcast
        self.check_propose_saved(round, propose_hash);
        self.blockchain.save_message(round, prevote.clone());

        trace!("Broadcast prevote: {:?}", prevote);
        self.broadcast(prevote);

        has_majority_prevotes
    }

    /// Broadcasts the `Precommit` message to all peers.
    pub fn broadcast_precommit(&mut self, round: Round, propose_hash: &Hash, block_hash: &Hash) {
        let validator_id = self
            .state
            .validator_id()
            .expect("called broadcast_precommit in Auditor node.");
        let precommit = self.sign_message(Precommit::new(
            validator_id,
            self.state.height(),
            round,
            propose_hash,
            block_hash,
            self.system_state.current_time().into(),
        ));
        self.state.add_precommit(precommit.clone());

        // Put our Precommit to the consensus cache before broadcast
        self.blockchain.save_message(round, precommit.clone());

        trace!("Broadcast precommit: {:?}", precommit);
        self.broadcast(precommit);
    }

    /// Checks that pre-commits count is correct and calls `verify_precommit` for each of them.
    fn verify_precommits(
        &self,
        precommits: &[Signed<Precommit>],
        block_hash: &Hash,
        block_height: Height,
    ) -> Result<(), failure::Error> {
        if precommits.len() < self.state.majority_count() {
            bail!("Received block without consensus");
        } else if precommits.len() > self.state.validators().len() {
            bail!("Wrong precommits count in block");
        }

        let mut validators = HashSet::with_capacity(precommits.len());
        let round = precommits[0].round();
        for precommit in precommits {
            if !validators.insert(precommit.validator()) {
                bail!("Several precommits from one validator in block")
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
        precommit: &Signed<Precommit>,
    ) -> Result<(), failure::Error> {
        if let Some(pub_key) = self.state.consensus_public_key_of(precommit.validator()) {
            if pub_key != precommit.author() {
                bail!(
                    "Received precommit with different validator id,\
                     validator_id = {}, validator_key: {:?},\
                     author_key = {:?}",
                    precommit.validator(),
                    pub_key,
                    precommit.author()
                )
            }
            if precommit.block_hash() != block_hash {
                bail!(
                    "Received precommit with wrong block_hash, precommit={:?}",
                    precommit
                )
            }
            if precommit.height() != block_height {
                bail!(
                    "Received precommit with wrong height, precommit={:?}",
                    precommit
                )
            }
            if precommit.round() != precommit_round {
                bail!(
                    "Received precommits with the different rounds, precommit={:?}",
                    precommit
                )
            }
        } else {
            bail!(
                "Received precommit with wrong validator, precommit={:?}",
                precommit
            )
        }
        Ok(())
    }

    /// Checks whether Propose is saved to the consensus cache and saves it otherwise
    fn check_propose_saved(&mut self, round: Round, propose_hash: &Hash) {
        if let Some(propose_state) = self.state.propose_mut(propose_hash) {
            if !propose_state.is_saved() {
                self.blockchain
                    .save_message(round, propose_state.message().clone());
                propose_state.set_saved(true);
            }
        }
    }
}
