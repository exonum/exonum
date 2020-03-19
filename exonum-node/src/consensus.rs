// Copyright 2020 The Exonum Team
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

use anyhow::{bail, format_err};
use exonum::{
    blockchain::{
        BlockContents, BlockKind, BlockParams, BlockPatch, Blockchain, BlockchainMut,
        PersistentPool, ProposerId, Schema, TransactionCache,
    },
    crypto::{Hash, PublicKey},
    helpers::{Height, Round, ValidatorId},
    merkledb::{BinaryValue, Fork, ObjectHash},
    messages::{AnyTx, Precommit, SignedMessage, Verified},
    runtime::ExecutionError,
};
use log::{error, info, trace, warn};

use std::{collections::HashSet, convert::TryFrom, fmt};

use crate::{
    events::InternalRequest,
    messages::{
        BlockRequest, BlockResponse, Consensus as ConsensusMessage, PoolTransactionsRequest,
        Prevote, PrevotesRequest, Propose, ProposeRequest, TransactionsRequest,
        TransactionsResponse,
    },
    proposer::{ProposeParams, ProposeTemplate},
    schema::NodeSchema,
    state::{IncompleteBlock, ProposeState, RequestData},
    NodeHandler,
};

/// Shortcut to get verified messages from bytes.
fn into_verified<T: TryFrom<SignedMessage>>(raw: &[Vec<u8>]) -> anyhow::Result<Vec<Verified<T>>> {
    let mut items = Vec::with_capacity(raw.len());
    for bytes in raw {
        let verified = SignedMessage::from_bytes(bytes.into())?.into_verified()?;
        items.push(verified);
    }
    Ok(items)
}

/// Helper trait to efficiently merge changes to the `BlockchainMut`.
trait PersistChanges {
    /// Persists changes to the node schema.
    fn persist_changes<F>(&mut self, change: F, error_msg: &str)
    where
        F: FnOnce(&mut NodeSchema<&Fork>);
}

impl PersistChanges for BlockchainMut {
    fn persist_changes<F>(&mut self, change: F, error_msg: &str)
    where
        F: FnOnce(&mut NodeSchema<&Fork>),
    {
        let fork = self.fork();
        change(&mut NodeSchema::new(&fork));
        self.merge(fork.into_patch()).expect(error_msg);
    }
}

/// Result of an action within a round.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RoundAction {
    /// New height was achieved.
    NewEpoch,
    /// No actions happened.
    None,
}

/// Error that may occur during transaction processing in `NodeHandler::handle_tx()`.
#[derive(Debug)]
pub(crate) enum HandleTxError {
    /// Transaction is committed in one of blocks.
    AlreadyProcessed,
    /// Transaction is invalid according to `Blockchain::check_tx`.
    Invalid(ExecutionError),
}

impl fmt::Display for HandleTxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyProcessed => formatter.write_str("Transaction is already processed"),
            Self::Invalid(e) => write!(formatter, "Transaction failed preliminary checks: {}", e),
        }
    }
}

// TODO Reduce view invocations. (ECR-171)
impl NodeHandler {
    /// Validates consensus message, then redirects it to the corresponding `handle_...` function.
    pub(crate) fn handle_consensus(&mut self, msg: ConsensusMessage) {
        if !self.is_enabled {
            trace!(
                "Ignoring a consensus message {:?} because the node is disabled",
                msg
            );
            return;
        }

        // Warning for messages from previous and future height
        if msg.epoch() < self.state.epoch().previous() || msg.epoch() > self.state.epoch().next() {
            warn!(
                "Received consensus message from other height: msg.height={}, self.height={}",
                msg.epoch(),
                self.state.epoch()
            );
        }

        // Ignore messages from previous and future height
        if msg.epoch() < self.state.epoch() || msg.epoch() > self.state.epoch().next() {
            return;
        }

        // Queued messages from next height or round
        // TODO: Should we ignore messages from far rounds? (ECR-171)
        if msg.epoch() == self.state.epoch().next() || msg.round() > self.state.round() {
            trace!(
                "Received consensus message from future round: msg.height={}, msg.round={}, \
                 self.height={}, self.round={}",
                msg.epoch(),
                msg.round(),
                self.state.epoch(),
                self.state.round()
            );
            let validator = msg.validator();
            let round = msg.round();
            self.state.add_queued(msg);
            trace!("Trying to reach actual round.");
            if let Some(r) = self.state.update_validator_round(validator, round) {
                trace!("Scheduling jump to round.");
                let height = self.state.epoch();
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
    fn handle_propose(&mut self, from: PublicKey, msg: &Verified<Propose>) {
        debug_assert_eq!(
            Some(from),
            self.state.consensus_public_key_of(msg.payload().validator)
        );

        if msg.payload().skip && !msg.payload().transactions.is_empty() {
            error!(
                "Received no-op propose with non-empty transaction list: {:?}",
                msg
            );
            return;
        }

        // Check prev_hash
        if msg.payload().prev_hash != self.state.last_hash() {
            error!("Received propose with wrong last_block_hash msg={:?}", msg);
            return;
        }

        // Check leader
        if msg.payload().validator != self.state.leader(msg.payload().round) {
            error!(
                "Wrong propose leader detected: actual={}, expected={}",
                msg.payload().validator,
                self.state.leader(msg.payload().round)
            );
            return;
        }

        trace!("Handle propose");

        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
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

        let hash = msg.object_hash();
        let known_nodes = self.remove_request(&RequestData::Propose(hash));

        if has_unknown_txs {
            trace!("REQUEST TRANSACTIONS");
            self.request(RequestData::ProposeTransactions(hash), from);
            for node in known_nodes {
                self.request(RequestData::ProposeTransactions(hash), node);
            }
        } else {
            self.handle_full_propose(hash, msg.payload().round);
        }
    }

    /// Validates a `BlockResponse`. Returns list of precommits authenticating the block, or
    /// an error if the block is invalid.
    fn validate_block_response(
        &self,
        msg: &Verified<BlockResponse>,
    ) -> anyhow::Result<Vec<Verified<Precommit>>> {
        if msg.payload().to != self.state.keys().consensus_pk() {
            bail!(
                "Received block intended for another peer, to={}, from={}",
                msg.payload().to.to_hex(),
                msg.author().to_hex()
            );
        }

        if !self.state.connect_list().is_peer_allowed(&msg.author()) {
            bail!(
                "Received request message from peer = {} which not in ConnectList.",
                msg.author().to_hex()
            );
        }

        let block = &msg.payload().block;
        let block_hash = block.object_hash();
        let epoch = block
            .epoch()
            .ok_or_else(|| format_err!("Received block without `epoch` field"))?;

        // TODO: Add block with greater height to queue. (ECR-171)
        let is_next_height = self.state.blockchain_height() == block.height;
        let is_future_epoch =
            self.state.blockchain_height() == block.height.next() && self.state.epoch() <= epoch;
        if !is_next_height && !is_future_epoch {
            bail!(
                "Received block has inappropriate height or epoch, msg={:?}",
                msg
            );
        }

        // Check block content.
        if block.prev_hash != self.last_block_hash() {
            bail!(
                "Received block prev_hash is distinct from the one in db, \
                 block={:?}, block.prev_hash={:?}, db.last_block_hash={:?}",
                msg,
                block.prev_hash,
                self.last_block_hash()
            );
        }

        if self.state.incomplete_block().is_some() {
            bail!("Already there is an incomplete block, msg={:?}", msg);
        }
        if block.get_header::<ProposerId>()?.is_none() {
            bail!("Received block without `proposer_id` header");
        }
        if block.is_skip() && !msg.payload().transactions.is_empty() {
            bail!("Received block skip with non-empty transactions");
        }
        if !msg.payload().verify_tx_hash() {
            bail!("Received block has invalid tx_hash, msg={:?}", msg);
        }

        let precommits = into_verified(&msg.payload().precommits)?;
        self.validate_precommits(&precommits, epoch, block_hash)?;
        Ok(precommits)
    }

    /// Handles the `BlockResponse` message.
    ///
    /// The message is first verified via `validate_block_response`. If the verification fails,
    /// this is logged and no further processing is performed.
    ///
    /// The node then remembers the block, waiting until the node knows all transactions in the block.
    /// If the node knows all transactions right away, it executes and commits the block.
    /// (The execution stage may be skipped if the block was executed earlier.)
    pub(crate) fn handle_block(&mut self, msg: Verified<BlockResponse>) {
        let precommits = match self.validate_block_response(&msg) {
            Ok(precommits) => precommits,
            Err(e) => {
                log::error!("Received incorrect block {:?}: {}", msg.payload(), e);
                return;
            }
        };
        // At this point, the block is valid.

        let sender = msg.author();
        let BlockResponse {
            block,
            transactions,
            ..
        } = msg.into_payload();
        let block_hash = block.object_hash();

        if self.state.block(&block_hash).is_none() {
            let block_height = block.height;
            let incomplete_block = IncompleteBlock::new(block, transactions, precommits);
            let snapshot = self.blockchain.snapshot();
            let txs_pool = Schema::new(snapshot.as_ref()).transactions_pool();

            let has_unknown_txs = self
                .state
                .create_incomplete_block(incomplete_block, &snapshot, &txs_pool)
                .has_unknown_txs();

            let known_nodes = self.remove_request(&RequestData::Block(block_height));

            if has_unknown_txs {
                trace!("REQUEST TRANSACTIONS");
                self.request(RequestData::BlockTransactions, sender);
                for node in known_nodes {
                    self.request(RequestData::BlockTransactions, node);
                }
            } else {
                // We know all transactions in the block!
                self.handle_full_block();
            }
        } else {
            self.commit(block_hash, precommits.into_iter(), None);
            self.request_next_block();
        }
    }

    /// Checks if propose is correct (doesn't contain invalid transactions), and then
    /// broadcasts a prevote for this propose.
    ///
    /// Returns `true` if majority of prevotes is achieved, and returns `false` otherwise.
    fn check_propose_and_broadcast_prevote(&mut self, round: Round, propose_hash: Hash) -> bool {
        // Do not send a prevote if propose contains incorrect transactions.
        let propose_state = self.state.propose(&propose_hash).unwrap_or_else(|| {
            panic!(
                "BUG: We're attempting to send a prevote, but don't have a propose; \
                 this should never occur. Round: {:?}, propose hash: {:?}, height: {:?}",
                round,
                propose_hash,
                self.state.epoch()
            )
        });
        if propose_state.has_invalid_txs() {
            warn!("Denying sending a prevote for a propose which contains incorrect transactions");
            self.state.has_majority_prevotes(round, propose_hash)
        } else {
            // Propose state is OK, send prevote.
            self.broadcast_prevote(round, propose_hash)
        }
    }

    /// Checks if propose is correct (doesn't contain invalid transactions), and then
    /// broadcasts a precommit for this propose.
    fn check_propose_and_broadcast_precommit(
        &mut self,
        round: Round,
        propose_hash: Hash,
        block_hash: Hash,
    ) {
        // Do not send a precommit if propose contains incorrect transactions.
        let propose_state = self.state.propose(&propose_hash).unwrap_or_else(|| {
            panic!(
                "BUG: We're attempting to send a precommit, but don't have a propose; \
                 this should never occur. Round: {:?}, propose hash: {:?}, height: {:?}",
                round,
                propose_hash,
                self.state.epoch()
            )
        });
        if propose_state.has_invalid_txs() {
            warn!(
                "Denying sending a precommit for a propose which contains incorrect transactions"
            );
        } else {
            // Propose state is OK, send precommit.
            self.broadcast_precommit(round, propose_hash, block_hash)
        }
    }

    /// Executes and commits block. This function is called when node has full propose information.
    ///
    /// # Panics
    ///
    /// This function panics if the hash from precommit doesn't match the calculated one.
    fn handle_full_propose(&mut self, hash: Hash, propose_round: Round) -> RoundAction {
        // Send prevote
        if self.state.locked_round() == Round::zero() {
            if self.state.is_validator() && !self.state.have_prevote(propose_round) {
                self.check_propose_and_broadcast_prevote(propose_round, hash);
            } else {
                // TODO: what if we HAVE prevote for the propose round? (ECR-171)
            }
        }

        // Lock to propose
        // TODO: avoid loop here (ECR-171).
        let start_round = std::cmp::max(self.state.locked_round().next(), propose_round);
        for round in start_round.iter_to(self.state.round().next()) {
            if self.state.has_majority_prevotes(round, hash) {
                let action = self.handle_majority_prevotes(round, hash);
                if action == RoundAction::NewEpoch {
                    return action;
                }
            }
        }

        // If this propose was confirmed by majority of nodes before, we can commit
        // this block right now.
        if let Some((round, block_hash)) = self.state.take_confirmed_propose(&hash) {
            // Execute block and get state hash
            let our_block_hash = self.execute(&hash);

            assert_eq!(
                our_block_hash, block_hash,
                "handle_full_propose: wrong block hash. Either a node's implementation is \
                 incorrect or validators majority works incorrectly."
            );

            let precommits = self
                .state
                .precommits(round, our_block_hash)
                .to_vec()
                .into_iter();
            self.commit(block_hash, precommits, Some(propose_round));
            return RoundAction::NewEpoch;
        }

        RoundAction::None
    }

    /// Executes and commits block. This function is called when node has full block information.
    ///
    /// # Panics
    ///
    /// Panics if the received block has incorrect `block_hash`.
    fn handle_full_block(&mut self) {
        // We suppose that the block doesn't contain incorrect transactions,
        // since `self.state` checks for it while creating an `IncompleteBlock`.
        let IncompleteBlock {
            header,
            precommits,
            transactions,
            ..
        } = self.state.take_completed_block();

        let block_hash = header.object_hash();
        if self.state.block(&block_hash).is_none() {
            let proposer_id = header
                .get_header::<ProposerId>()
                .expect("`ProposerId` header should be checked in `validate_block_response`")
                .expect("`ProposerId` header should be checked in `validate_block_response`");
            let epoch = header
                .epoch()
                .expect("`epoch` header should be checked in `validate_block_response`");

            let block_contents = if header.is_skip() {
                BlockContents::Skip
            } else {
                BlockContents::Transactions(&transactions)
            };
            let patch = self.create_block(proposer_id, epoch, block_contents);
            let computed_block_hash = patch.block_hash();

            // Verify `block_hash`.
            assert_eq!(
                computed_block_hash, block_hash,
                "Block_hash incorrect in the received block={:?}. Either a node's \
                 implementation is incorrect or validators majority works incorrectly",
                header
            );

            self.state
                .add_block(patch, transactions, proposer_id, epoch);
        }

        self.commit(block_hash, precommits.into_iter(), None);
        self.request_next_block();
    }

    /// Handles the `Prevote` message. For details see the message documentation.
    fn handle_prevote(&mut self, from: PublicKey, msg: &Verified<Prevote>) {
        trace!("Handle prevote");

        debug_assert_eq!(
            Some(from),
            self.state.consensus_public_key_of(msg.payload().validator)
        );

        // Add prevote and check if majority of validator nodes have voted for this propose.
        let has_consensus = self.state.add_prevote(msg.clone());

        // Request propose or transactions if needed.
        let has_propose_with_txs = self.request_propose_or_txs(msg.payload().propose_hash, from);

        // Request prevotes if this propose corresponds to the bigger round.
        if msg.payload().locked_round > self.state.locked_round() {
            self.request(
                RequestData::Prevotes(msg.payload().locked_round, msg.payload().propose_hash),
                from,
            );
        }

        // Lock to propose.
        if has_consensus && has_propose_with_txs {
            self.handle_majority_prevotes(msg.payload().round, msg.payload().propose_hash);
        }
    }

    /// Locks to the propose by calling `lock`. This function is called when node receives
    /// +2/3 pre-votes.
    fn handle_majority_prevotes(
        &mut self,
        prevote_round: Round,
        propose_hash: Hash,
    ) -> RoundAction {
        // Remove request info.
        self.remove_request(&RequestData::Prevotes(prevote_round, propose_hash));
        // Lock to propose.
        if self.state.locked_round() < prevote_round && self.state.propose(&propose_hash).is_some()
        {
            // Check that propose is valid and should be executed.
            let propose_state = self.state.propose(&propose_hash).unwrap();
            if propose_state.has_invalid_txs() {
                panic!(
                    "handle_majority_prevotes: propose contains invalid transaction(s). \
                     Either a node's implementation is incorrect \
                     or validators majority works incorrectly"
                );
            }

            self.lock(prevote_round, propose_hash)
        } else {
            RoundAction::None
        }
    }

    /// Executes and commits block. This function is called when the node has +2/3 pre-commits.
    ///
    /// # Panics
    ///
    /// This method panics if:
    /// - Accepted propose contains transaction(s) for which `BlockchainMut::check_tx` failed.
    /// - Calculated hash of the block doesn't match the hash from precommits.
    fn handle_majority_precommits(
        &mut self,
        round: Round,
        propose_hash: &Hash,
        block_hash: &Hash,
    ) -> RoundAction {
        // If we don't know this propose yet, it means that we somehow missed a propose
        // broadcast and the whole voting process. Nevertheless, we will store this propose
        // in the list of confirmed proposes, so we will be able to commit the block once the
        // information about the propose is known.
        let maybe_propose_state = self.state.propose(propose_hash);
        if maybe_propose_state.map_or(true, ProposeState::has_unknown_txs) {
            self.state
                .add_propose_confirmed_by_majority(round, *propose_hash, *block_hash);
        }

        let propose_state = match self.state.propose(propose_hash) {
            Some(state) => state,
            None => return RoundAction::None,
        };

        // Check if we have all the transactions for this propose.
        if propose_state.has_unknown_txs() {
            // Some of transactions are missing, we can't commit the block right now.
            // Instead, request transactions from proposer.
            let proposer = self
                .state
                .consensus_public_key_of(propose_state.message().payload().validator)
                .unwrap();

            self.request(RequestData::ProposeTransactions(*propose_hash), proposer);
            return RoundAction::None;
        }

        // Check that propose is valid and should be executed.
        if propose_state.has_invalid_txs() {
            // Propose is known to have invalid transactions, but is confirmed by
            // the majority of nodes; we can't operate in those conditions.
            panic!(
                "handle_majority_precommits: propose contains invalid transaction(s). \
                 Either a node's implementation is incorrect \
                 or validators majority works incorrectly"
            );
        }

        // Execute block and verify that the block hash matches expected one.
        let our_block_hash = self.execute(propose_hash);
        assert_eq!(
            &our_block_hash, block_hash,
            "handle_majority_precommits: wrong block hash. Either a node's implementation is \
             incorrect or validators majority works incorrectly."
        );

        // Commit.
        let precommits = self.state.precommits(round, our_block_hash).to_vec();
        self.commit(our_block_hash, precommits.into_iter(), Some(round));

        RoundAction::NewEpoch
    }

    /// Locks node to the specified round, so pre-votes for the lower round will be ignored.
    fn lock(&mut self, prevote_round: Round, propose_hash: Hash) -> RoundAction {
        trace!("MAKE LOCK {:?} {:?}", prevote_round, propose_hash);
        for round in prevote_round.iter_to(self.state.round().next()) {
            // Here we have all the transactions from the propose, so
            // we should send a prevote if we didn't send it earlier.
            if self.state.is_validator() && !self.state.have_prevote(round) {
                self.check_propose_and_broadcast_prevote(round, propose_hash);
            }

            // Lock on the round and propose if we've received the majority of prevotes.
            if self.state.has_majority_prevotes(round, propose_hash) {
                // Put consensus messages for current `Propose` and this round to the cache.
                self.check_propose_saved(round, &propose_hash);
                let raw_messages = self
                    .state
                    .prevotes(prevote_round, propose_hash)
                    .iter()
                    .map(|p| p.clone().into());

                self.blockchain.persist_changes(
                    |schema| schema.save_messages(round, raw_messages),
                    "Cannot save consensus messages",
                );

                // Lock the state on the round and propose.
                self.state.lock(round, propose_hash);
                // Execute block and send precommit.
                if self.state.is_validator() && !self.state.have_incompatible_prevotes() {
                    // Execute block and get state hash.
                    let block_hash = self.execute(&propose_hash);
                    self.check_propose_and_broadcast_precommit(round, propose_hash, block_hash);
                    // Commit the block if it's approved by the majority of validators.
                    if self.state.has_majority_precommits(round, block_hash) {
                        return self.handle_majority_precommits(round, &propose_hash, &block_hash);
                    }
                }
                // Remove request info.
                self.remove_request(&RequestData::Prevotes(round, propose_hash));
            }
        }

        RoundAction::None
    }

    /// Handles the `Precommit` message. For details see the message documentation.
    fn handle_precommit(&mut self, from: PublicKey, msg: &Verified<Precommit>) {
        trace!("Handle precommit");

        debug_assert_eq!(
            Some(from),
            self.state.consensus_public_key_of(msg.payload().validator)
        );

        // Add precommit
        let has_consensus = self.state.add_precommit(msg.clone());

        // Request propose
        if self.state.propose(&msg.payload().propose_hash).is_none() {
            self.request(RequestData::Propose(msg.payload().propose_hash), from);
        }

        // Request prevotes
        // TODO: If Precommit sender in on a greater height, then it cannot have +2/3 prevotes.
        // So can we get rid of useless sending RequestPrevotes message? (ECR-171)
        if msg.payload().round > self.state.locked_round() {
            self.request(
                RequestData::Prevotes(msg.payload().round, msg.payload().propose_hash),
                from,
            );
        }

        // Has majority precommits
        if has_consensus {
            self.handle_majority_precommits(
                msg.payload().round,
                &msg.payload().propose_hash,
                &msg.payload().block_hash,
            );
        }
    }

    /// Commits block, so that the new height is achieved.
    fn commit<I: Iterator<Item = Verified<Precommit>>>(
        &mut self,
        block_hash: Hash,
        precommits: I,
        round: Option<Round>,
    ) {
        let mut block_state = self.state.take_block_for_commit(&block_hash);
        let block_kind = block_state.kind();
        let block_epoch = block_state.epoch();
        let committed_txs = block_state.txs();
        let proposer = block_state.proposer_id();

        trace!("COMMIT {:?} {:?}", block_kind, block_hash);

        // Remove committed transactions from the cache.
        for tx_hash in committed_txs {
            self.state.tx_cache_mut().remove(tx_hash);
        }
        let committed_txs_len = committed_txs.len();

        // Consensus messages cache is useful only during one height, so it should be
        // cleared when a new height is achieved.
        self.blockchain.persist_changes(
            |schema| schema.consensus_messages_cache().clear(),
            "Cannot clear consensus messages",
        );

        self.blockchain
            .commit(block_state.patch(), precommits)
            .expect("Cannot commit block");

        match block_kind {
            BlockKind::Normal => {
                // Update node state.
                self.state
                    .update_config(Schema::new(&self.blockchain.snapshot()).consensus_config());
                // Update state to new height.
                self.state.new_height(
                    block_hash,
                    block_epoch.next(),
                    self.system_state.current_time(),
                );

                let snapshot = self.blockchain.snapshot();
                for plugin in &self.plugins {
                    plugin.after_commit(&snapshot);
                }
            }

            BlockKind::Skip => {
                // Update state to the new epoch (but not a new height).
                self.state
                    .new_epoch(block_epoch.next(), self.system_state.current_time());
            }

            _ => unreachable!("No other block kinds are supported"),
        }

        let snapshot = self.blockchain.snapshot();
        let schema = Schema::new(&snapshot);
        let pool_len = schema.transactions_pool_len();
        let epoch = self.state.epoch();
        let blockchain_height = self.state.blockchain_height();
        info!(
            "COMMIT ====== epoch={}, height={}, proposer={}, round={}, \
             committed={}, pool={}, hash={}",
            epoch,
            blockchain_height,
            proposer,
            round.map_or_else(|| "?".to_owned(), |x| x.to_string()),
            committed_txs_len,
            pool_len,
            block_hash.to_hex(),
        );

        self.broadcast_status();
        self.add_status_timeout();

        // Add timeout for the first round.
        self.add_round_timeout();
        // Send propose if we are the round leader.
        if self.state.is_leader() {
            self.add_propose_timeout();
        }

        // Handle queued messages. Note that this will drop all messages if the node jumps
        // epochs (e.g., when it lags behind).
        for msg in self.state.queued() {
            self.handle_consensus(msg);
        }
    }

    /// Checks if the transaction is new and adds it to the pool. This may trigger an expedited
    /// `Propose` timeout on this node if transaction count in the pool goes over the threshold.
    ///
    /// Before adding a transaction into pool, this method calls `Blockchain::check_tx` to
    /// ensure that transaction passes at least basic checks. If `Blockchain::check_tx` fails,
    /// transaction will be considered invalid and not stored to the pool (instead, its hash will
    /// be stored in the temporary invalid messages set, so we will be able to detect a block/propose
    /// with an invalid tx later; note that the temporary set is cleared every block).
    ///
    /// # Panics
    ///
    /// This function panics if it receives an invalid transaction for an already committed block.
    pub(crate) fn handle_tx(&mut self, msg: Verified<AnyTx>) -> Result<(), HandleTxError> {
        let hash = msg.object_hash();

        let snapshot = self.blockchain.snapshot();
        let tx_pool = PersistentPool::new(&snapshot, self.state.tx_cache());
        if tx_pool.contains_transaction(hash) {
            return Err(HandleTxError::AlreadyProcessed);
        }

        let outcome;
        if let Err(e) = Blockchain::check_tx(&snapshot, &msg) {
            // Store transaction as invalid to know it if it'll be included into a proposal.
            // Please note that it **must** happen before calling `check_incomplete_proposes`,
            // since the latter uses `invalid_txs` to recalculate the validity of proposals.
            self.state.invalid_txs_mut().insert(msg.object_hash());

            // Since the transaction, despite being incorrect, is received from within the
            // network, we have to deal with it. We don't consider the transaction unknown
            // anymore, but we've marked it as incorrect, and if it'll be a part of the block,
            // we will be able to panic.
            // Thus, we don't stop the execution here.
            outcome = Err(HandleTxError::Invalid(e));
        } else {
            // Transaction is OK, store it to the cache or persistent pool.
            if self.state.persist_txs_immediately() {
                let fork = self.blockchain.fork();
                Schema::new(&fork).add_transaction_into_pool(msg);
                self.blockchain
                    .merge(fork.into_patch())
                    .expect("Cannot add transaction to persistent pool");
            } else {
                self.state.tx_cache_mut().insert(hash, msg);
            }
            outcome = Ok(());
        }

        if self.state.is_leader() && self.state.round() != Round::zero() {
            self.maybe_add_propose_timeout();
        }

        // We can collect the transactions in three possible scenarios:
        // 1. We're participating in the consensus and should vote for the block.
        // 2. We're lagging behind and processing committed blocks to achieve the current height.
        // 3. We're an auditor and should just execute committed blocks.

        // Scenario 1: A new block is being created, process the consensus routine.
        let full_proposes = self.state.check_incomplete_proposes(hash);
        // Go to handle full propose if we get last transaction.
        let mut height_bumped = false;
        for (hash, round) in full_proposes {
            self.remove_request(&RequestData::ProposeTransactions(hash));
            // If a new height was achieved, no more proposals for this height
            // should be processed. However, we still have to remove requests.
            if !height_bumped {
                height_bumped = self.handle_full_propose(hash, round) == RoundAction::NewEpoch;
            }
        }

        // Scenarios 2 and 3: We're processing an already committed block.
        // Note that this scenario should be mutually exclusive with the scenario 1:
        // if our height is not the height of the blockchain, validator nodes do not
        // process the consensus messages.
        let action = self.state.remove_unknown_transaction(hash);
        // Go to handle full block if we've got the last necessary transaction.
        if action == RoundAction::NewEpoch {
            self.remove_request(&RequestData::BlockTransactions);
            self.handle_full_block();
        }
        outcome
    }

    /// Handles raw transactions.
    pub(crate) fn handle_txs_batch(
        &mut self,
        msg: &Verified<TransactionsResponse>,
    ) -> anyhow::Result<()> {
        if msg.payload().to != self.state.keys().consensus_pk() {
            bail!(
                "Received response intended for another peer, to={}, from={}",
                msg.payload().to.to_hex(),
                msg.author().to_hex()
            )
        }

        if !self.state.connect_list().is_peer_allowed(&msg.author()) {
            bail!(
                "Received response message from peer = {} which not in ConnectList.",
                msg.author().to_hex()
            )
        }
        for tx in &msg.payload().transactions {
            self.execute_later(InternalRequest::VerifyMessage(tx.to_owned()));
        }
        Ok(())
    }

    /// Handles external boxed transaction. If the transaction is considered valid by the node,
    /// it will be broadcast to the peers.
    pub(crate) fn handle_incoming_tx(&mut self, msg: Verified<AnyTx>) {
        trace!("Handle incoming transaction");

        match self.handle_tx(msg.clone()) {
            Ok(()) => self.broadcast(msg),
            Err(e) => log::warn!(
                "Failed to process transaction {:?} received via `ApiSender`: {}",
                msg.payload(),
                e
            ),
        }
    }

    /// Handle new round, after jump.
    pub(crate) fn handle_new_round(&mut self, height: Height, round: Round) {
        trace!("Handle new round");
        if height != self.state.epoch() {
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
                let has_majority_prevotes = self.check_propose_and_broadcast_prevote(round, hash);
                if has_majority_prevotes {
                    self.handle_majority_prevotes(round, hash);
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
    pub(crate) fn handle_round_timeout(&mut self, height: Height, round: Round) {
        // TODO: Debug asserts? (ECR-171)
        if height != self.state.epoch() {
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
    pub(crate) fn handle_propose_timeout(&mut self, height: Height, round: Round) {
        // TODO debug asserts (ECR-171)?
        if height != self.state.epoch() {
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

        let validator_id = if let Some(validator_id) = self.state.validator_id() {
            validator_id
        } else {
            return;
        };
        let round = self.state.round();

        let propose_template = self.get_propose_template();
        let propose = match propose_template {
            ProposeTemplate::Ordinary { tx_hashes } => Propose::new(
                validator_id,
                self.state.epoch(),
                round,
                self.state.last_hash(),
                tx_hashes,
            ),

            ProposeTemplate::Skip => Propose::skip(
                validator_id,
                self.state.epoch(),
                round,
                self.state.last_hash(),
            ),
        };
        let propose = self.sign_message(propose);

        // Put our propose to the consensus messages cache.
        self.blockchain.persist_changes(
            |schema| schema.save_message(round, propose.clone()),
            "Cannot save `Propose` to message cache",
        );

        trace!("Broadcast propose: {:?}", propose);
        self.broadcast(propose.clone());
        self.allow_expedited_propose = true;

        // Save our propose into state
        let hash = self.state.add_self_propose(propose);

        // Send prevote
        let has_majority_prevotes = self.check_propose_and_broadcast_prevote(round, hash);
        if has_majority_prevotes {
            self.handle_majority_prevotes(round, hash);
        }
    }

    fn get_propose_template(&mut self) -> ProposeTemplate {
        let txs_cache_len = self.state.tx_cache_len() as u64;
        info!("LEADER: cache = {}", txs_cache_len);

        let snapshot = self.blockchain.snapshot();
        let pool = PersistentPool::new(snapshot.as_ref(), self.state.tx_cache());
        let params = ProposeParams::new(self.state(), &snapshot);
        self.block_proposer.propose_block(pool, params)
    }

    /// Handles request timeout by sending the corresponding request message to a peer.
    pub(crate) fn handle_request_timeout(&mut self, data: &RequestData, peer: Option<PublicKey>) {
        trace!("HANDLE REQUEST TIMEOUT");
        // FIXME: Check height? (ECR-171)
        if let Some(peer) = self.state.retry(data, peer) {
            self.add_request_timeout(data.clone(), Some(peer));

            if !self.is_enabled {
                trace!(
                    "Not sending a request {:?} because the node is paused.",
                    data
                );
                return;
            }

            let message: SignedMessage = match *data {
                RequestData::Propose(propose_hash) => self
                    .sign_message(ProposeRequest::new(peer, self.state.epoch(), propose_hash))
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
                    self.sign_message(TransactionsRequest::new(peer, txs))
                        .into()
                }

                RequestData::PoolTransactions => {
                    self.sign_message(PoolTransactionsRequest::new(peer)).into()
                }

                RequestData::BlockTransactions => {
                    let txs: Vec<_> = match self.state.incomplete_block() {
                        Some(incomplete_block) => {
                            incomplete_block.unknown_txs().iter().cloned().collect()
                        }
                        None => return,
                    };
                    self.sign_message(TransactionsRequest::new(peer, txs))
                        .into()
                }

                RequestData::Prevotes(round, propose_hash) => self
                    .sign_message(PrevotesRequest::new(
                        peer,
                        self.state.epoch(),
                        round,
                        propose_hash,
                        self.state.known_prevotes(round, propose_hash),
                    ))
                    .into(),

                RequestData::Block(height) => {
                    self.sign_message(BlockRequest::new(peer, height)).into()
                }

                RequestData::BlockOrEpoch {
                    block_height,
                    epoch,
                } => self
                    .sign_message(BlockRequest::with_epoch(peer, block_height, epoch))
                    .into(),
            };

            trace!("Send request {:?} to peer {:?}", data, peer);
            self.send_to_peer(peer, message);
        }
    }

    /// Creates block with given transaction and returns its hash and corresponding changes.
    fn create_block(
        &mut self,
        proposer_id: ValidatorId,
        epoch: Height,
        contents: BlockContents<'_>,
    ) -> BlockPatch {
        let block_params = BlockParams::with_contents(contents, proposer_id, epoch);
        self.blockchain
            .create_patch(block_params, self.state.tx_cache())
    }

    /// Calls `create_block` with transactions from the corresponding `Propose` and returns the
    /// block hash.
    fn execute(&mut self, propose_hash: &Hash) -> Hash {
        // if we already execute this block, return hash
        let propose_state = self.state.propose(propose_hash).unwrap();
        let block_kind = propose_state.block_kind();
        if let Some(hash) = propose_state.block_hash() {
            return hash;
        }

        let propose = propose_state.message().payload().to_owned();
        let block_contents = match block_kind {
            BlockKind::Normal => BlockContents::Transactions(&propose.transactions),
            BlockKind::Skip => BlockContents::Skip,
            _ => unreachable!("No other block kinds are supported"),
        };
        let patch = self.create_block(propose.validator, propose.epoch, block_contents);
        let block_hash = patch.block_hash();
        self.state.add_block(
            patch,
            propose.transactions,
            propose.validator,
            propose.epoch,
        );
        self.state
            .propose_mut(propose_hash)
            .unwrap()
            .set_block_hash(block_hash);
        block_hash
    }

    /// Returns `true` if propose and all transactions are known, otherwise requests needed data
    /// and returns `false`.
    fn request_propose_or_txs(&mut self, propose_hash: Hash, key: PublicKey) -> bool {
        let requested_data = match self.state.propose(&propose_hash) {
            Some(state) => {
                // Request transactions
                if state.has_unknown_txs() {
                    Some(RequestData::ProposeTransactions(propose_hash))
                } else {
                    None
                }
            }
            None => {
                // Request propose
                Some(RequestData::Propose(propose_hash))
            }
        };

        if let Some(data) = requested_data {
            self.request(data, key);
            false
        } else {
            true
        }
    }

    /// Requests a block for the next height from all peers with a bigger height. Called when the
    /// node tries to catch up with other nodes' height.
    pub(crate) fn request_next_block(&mut self) {
        if !self.is_enabled {
            trace!("Not sending a request for the next block because the node is paused.");
            return;
        }

        // TODO: Randomize next peer. (ECR-171)
        let peers = self.state.advanced_peers();
        if let Some((peer, data)) = peers.send_message(&self.state) {
            self.request(data, peer);
        }
    }

    /// Removes the specified request from the pending request list.
    fn remove_request(&mut self, data: &RequestData) -> HashSet<PublicKey> {
        // TODO: Clear timeout. (ECR-171)
        self.state.remove_request(data)
    }

    /// Broadcasts the `Prevote` message to all peers.
    fn broadcast_prevote(&mut self, round: Round, propose_hash: Hash) -> bool {
        let validator_id = self
            .state
            .validator_id()
            .expect("called broadcast_prevote in Auditor node.");
        let locked_round = self.state.locked_round();
        let prevote = self.sign_message(Prevote::new(
            validator_id,
            self.state.epoch(),
            round,
            propose_hash,
            locked_round,
        ));
        let has_majority_prevotes = self.state.add_prevote(prevote.clone());

        // save outgoing Prevote to the consensus messages cache before broadcast
        self.check_propose_saved(round, &propose_hash);
        self.blockchain.persist_changes(
            |schema| schema.save_message(round, prevote.clone()),
            "Cannot save `Prevote` to message cache",
        );

        trace!("Broadcast prevote: {:?}", prevote);
        self.broadcast(prevote);

        has_majority_prevotes
    }

    /// Broadcasts the `Precommit` message to all peers.
    fn broadcast_precommit(&mut self, round: Round, propose_hash: Hash, block_hash: Hash) {
        let validator_id = self
            .state
            .validator_id()
            .expect("called broadcast_precommit in Auditor node.");
        let precommit = self.sign_message(Precommit::new(
            validator_id,
            self.state.epoch(),
            round,
            propose_hash,
            block_hash,
            self.system_state.current_time().into(),
        ));
        self.state.add_precommit(precommit.clone());

        // Put our Precommit to the consensus cache before broadcast.
        self.blockchain.persist_changes(
            |schema| schema.save_message(round, precommit.clone()),
            "Cannot save `Precommit` to message cache",
        );

        trace!("Broadcast precommit: {:?}", precommit);
        self.broadcast(precommit);
    }

    /// Checks that pre-commits count is correct and calls `validate_precommit` for each of them.
    fn validate_precommits(
        &self,
        precommits: &[Verified<Precommit>],
        epoch: Height,
        block_hash: Hash,
    ) -> anyhow::Result<()> {
        if precommits.len() < self.state.majority_count() {
            bail!("Received block without consensus");
        } else if precommits.len() > self.state.validators().len() {
            bail!("Wrong precommits count in block");
        }

        let mut validators = HashSet::with_capacity(precommits.len());
        let round = precommits[0].payload().round;
        for precommit in precommits {
            if !validators.insert(precommit.payload().validator) {
                bail!("Several precommits from one validator in block");
            }
            self.validate_precommit(block_hash, epoch, round, precommit)?;
        }

        Ok(())
    }

    /// Verifies that `Precommit` contains correct block hash, height round and is signed by the
    /// right validator.
    fn validate_precommit(
        &self,
        block_hash: Hash,
        epoch: Height,
        precommit_round: Round,
        precommit: &Verified<Precommit>,
    ) -> anyhow::Result<()> {
        let precommit_author = precommit.author();
        let precommit = precommit.payload();
        if let Some(pub_key) = self.state.consensus_public_key_of(precommit.validator) {
            if pub_key != precommit_author {
                bail!(
                    "Received precommit with different validator id, \
                     validator_id = {}, validator_key: {:?}, \
                     author_key = {:?}",
                    precommit.validator,
                    pub_key,
                    precommit_author
                );
            }
            if precommit.block_hash != block_hash {
                bail!(
                    "Received precommit with wrong block_hash, precommit={:?}",
                    precommit
                );
            }
            if precommit.epoch != epoch {
                bail!(
                    "Received precommit belonging to wrong epoch, precommit={:?}",
                    precommit
                );
            }
            if precommit.round != precommit_round {
                bail!(
                    "Received precommit with the different rounds, precommit={:?}",
                    precommit
                );
            }
        } else {
            bail!(
                "Received precommit with wrong validator, precommit={:?}",
                precommit
            );
        }
        Ok(())
    }

    /// Checks whether `Propose` is saved to the consensus cache and saves it otherwise.
    fn check_propose_saved(&mut self, round: Round, propose_hash: &Hash) {
        if let Some(propose_state) = self.state.propose_mut(propose_hash) {
            if !propose_state.is_saved() {
                self.blockchain.persist_changes(
                    |schema| schema.save_message(round, propose_state.message().clone()),
                    "Cannot save foreign `Propose` to message cache",
                );
                propose_state.set_saved(true);
            }
        }
    }
}
