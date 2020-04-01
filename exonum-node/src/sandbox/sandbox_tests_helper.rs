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

//! Functions with reusable code used for sandbox tests.

use bit_vec::BitVec;
use exonum::{
    blockchain::{AdditionalHeaders, Block, Epoch, ProposerId},
    crypto::Hash,
    helpers::{Height, Round, ValidatorId},
    merkledb::{access::CopyAccessExt, Database, HashTag, ObjectHash, TemporaryDB},
    messages::{AnyTx, Precommit, Verified},
};
use log::trace;

use std::{cell::RefCell, collections::BTreeMap, time::Duration};

use super::{
    timestamping::{TimestampingTxGenerator, DATA_SIZE},
    Sandbox,
};
use crate::messages::{Prevote, PrevotesRequest, Propose, ProposeRequest};

pub type TimestampingSandbox = Sandbox;

pub const NOT_LOCKED: Round = Round(0);

type Milliseconds = u64;
pub const PROPOSE_TIMEOUT: Milliseconds = 200;

/// Idea of `BlockBuilder` is to implement the builder pattern in order to get `Block` with
/// default data from the sandbox and, possibly, update few fields with custom data.
#[derive(Debug)]
pub struct BlockBuilder<'a> {
    proposer_id: Option<ValidatorId>,
    height: Option<Height>,
    prev_hash: Option<Hash>,
    tx_hash: Option<Hash>,
    state_hash: Option<Hash>,
    error_hash: Option<Hash>,
    tx_count: Option<u32>,
    entries: Option<AdditionalHeaders>,

    sandbox: &'a TimestampingSandbox,
}

impl<'a> BlockBuilder<'a> {
    pub fn new(sandbox: &'a TimestampingSandbox) -> Self {
        BlockBuilder {
            proposer_id: None,
            height: None,
            prev_hash: None,
            tx_hash: None,
            state_hash: None,
            error_hash: None,
            tx_count: None,
            entries: None,
            sandbox,
        }
    }

    pub fn with_proposer_id(mut self, proposer_id: ValidatorId) -> Self {
        self.proposer_id = Some(proposer_id);
        self
    }

    pub fn with_tx_hash(mut self, individual_transaction_hash: &'a Hash) -> Self {
        self.tx_hash = Some(*individual_transaction_hash);
        self.tx_count = Some(1);
        self
    }

    pub fn with_txs_hashes(mut self, tx_hashes: &[Hash]) -> Self {
        // root of merkle table, containing this array of transactions
        let merkle_root = compute_txs_merkle_root(tx_hashes);
        self.tx_hash = Some(merkle_root);
        self.tx_count = Some(tx_hashes.len() as u32);
        self
    }

    pub fn with_state_hash(mut self, state_hash: &'a Hash) -> Self {
        self.state_hash = Some(*state_hash);
        self
    }

    pub fn with_error_hash(mut self, error_hash: &Hash) -> Self {
        self.error_hash = Some(*error_hash);
        self
    }

    pub fn build(&self) -> Block {
        let proposer_id = self
            .proposer_id
            .unwrap_or_else(|| self.sandbox.current_leader());

        let mut additional_headers = self.entries.clone().unwrap_or_else(AdditionalHeaders::new);
        additional_headers.insert::<ProposerId>(proposer_id);
        additional_headers.insert::<Epoch>(self.sandbox.current_epoch());

        Block {
            height: self.height.unwrap_or_else(|| self.sandbox.current_epoch()),
            tx_count: self.tx_count.unwrap_or(0),
            prev_hash: self.prev_hash.unwrap_or_else(|| self.sandbox.last_hash()),
            tx_hash: self.tx_hash.unwrap_or_else(HashTag::empty_list_hash),
            state_hash: self
                .state_hash
                .unwrap_or_else(|| self.sandbox.last_state_hash()),
            error_hash: self.error_hash.unwrap_or_else(HashTag::empty_map_hash),
            additional_headers,
        }
    }
}

// Idea of ProposeBuilder is to implement Builder pattern in order to get Propose with
// default data from sandbox and, possibly, update few fields with custom data.
#[derive(Debug)]
pub struct ProposeBuilder<'a> {
    validator_id: Option<ValidatorId>,
    height: Option<Height>,
    round: Option<Round>,
    prev_hash: Option<&'a Hash>,
    tx_hashes: Option<&'a [Hash]>,

    sandbox: &'a TimestampingSandbox,
}

impl<'a> ProposeBuilder<'a> {
    pub fn new(sandbox: &'a TimestampingSandbox) -> Self {
        ProposeBuilder {
            validator_id: None,
            height: None,
            round: None,
            prev_hash: None,
            tx_hashes: None,
            sandbox,
        }
    }

    pub fn with_validator(mut self, validator_id: ValidatorId) -> Self {
        self.validator_id = Some(validator_id);
        self
    }

    pub fn with_height(mut self, height: Height) -> Self {
        self.height = Some(height);
        self
    }

    pub fn with_round(mut self, round: Round) -> Self {
        self.round = Some(round);
        self
    }

    pub fn with_prev_hash(mut self, prev_hash: &'a Hash) -> Self {
        self.prev_hash = Some(prev_hash);
        self
    }

    pub fn with_tx_hashes(mut self, tx_hashes: &'a [Hash]) -> Self {
        self.tx_hashes = Some(tx_hashes);
        self
    }

    pub fn build(&self) -> Verified<Propose> {
        self.sandbox.create_propose(
            self.validator_id
                .unwrap_or_else(|| self.sandbox.current_leader()),
            self.height.unwrap_or_else(|| self.sandbox.current_epoch()),
            self.round.unwrap_or_else(|| self.sandbox.current_round()),
            *self.prev_hash.unwrap_or(&self.sandbox.last_hash()),
            self.tx_hashes.unwrap_or(&[]).iter().cloned(),
            self.sandbox.secret_key(
                self.validator_id
                    .unwrap_or_else(|| self.sandbox.current_leader()),
            ),
        )
    }
}

#[derive(Debug)]
pub struct SandboxState {
    pub accepted_propose_hash: RefCell<Hash>,
    pub accepted_block_hash: RefCell<Hash>,
    pub committed_transaction_hashes: RefCell<Vec<Hash>>,
    pub time_since_round_start: RefCell<Milliseconds>,
}

impl SandboxState {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for SandboxState {
    fn default() -> Self {
        Self {
            accepted_block_hash: RefCell::new(Hash::zero()),
            accepted_propose_hash: RefCell::new(Hash::zero()),
            committed_transaction_hashes: RefCell::new(Vec::new()),
            time_since_round_start: RefCell::new(0),
        }
    }
}

pub fn compute_txs_merkle_root(txs: &[Hash]) -> Hash {
    let fork = TemporaryDB::new().fork();
    let mut hashes = fork.get_proof_list("name");
    hashes.extend(txs.iter().cloned());
    hashes.object_hash()
}

pub fn add_round_with_transactions(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    transactions: &[Hash],
) -> Option<Verified<Propose>> {
    try_add_round_with_transactions(sandbox, sandbox_state, transactions, false).unwrap()
}

pub fn try_add_round_with_transactions(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    transactions: &[Hash],
    fast_track: bool,
) -> Result<Option<Verified<Propose>>, String> {
    let round_timeout = sandbox.current_round_timeout();

    trace!("-------------------------add_round_with_transactions started-------------------------");
    trace!("round before: {:?}", sandbox.current_round());
    trace!("sandbox_time: {:?}", sandbox.time());
    trace!("is_leader before time adding: {:?}", sandbox.is_leader());

    if sandbox.is_leader() {
        let res =
            try_check_and_broadcast_propose_and_prevote(sandbox, sandbox_state, transactions)?;
        if fast_track {
            if let Some(propose) = res {
                return Ok(Some(propose));
            }
        }
    }

    // How much time left till next round_timeout
    let time_till_next_round =
        round_timeout - *sandbox_state.time_since_round_start.borrow() % round_timeout;

    trace!("going to add {:?} millis", time_till_next_round);
    sandbox.add_time(Duration::from_millis(time_till_next_round));
    trace!("sandbox_time after adding: {:?}", sandbox.time());
    trace!("round after: {:?}", sandbox.current_round());
    trace!("sandbox.current_round: {:?}", sandbox.current_round());

    trace!("is_leader after time adding: {:?}", sandbox.is_leader());
    *sandbox_state.time_since_round_start.borrow_mut() = 0;

    if sandbox.is_leader() {
        try_check_and_broadcast_propose_and_prevote(sandbox, sandbox_state, transactions)
    } else {
        Ok(None)
    }
}

pub fn gen_timestamping_tx() -> Verified<AnyTx> {
    let mut tx_gen = TimestampingTxGenerator::new(DATA_SIZE);
    tx_gen.next().unwrap()
}

pub fn gen_incorrect_tx() -> Verified<AnyTx> {
    let mut tx_gen = TimestampingTxGenerator::for_incorrect_service(DATA_SIZE);
    tx_gen.next().unwrap()
}

pub fn add_one_height(sandbox: &TimestampingSandbox, sandbox_state: &SandboxState) {
    // Generate some transaction.
    let tx = gen_timestamping_tx();
    add_one_height_with_transactions(sandbox, sandbox_state, &[tx]);
}

pub fn add_one_height_with_transactions<'a, I>(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    txs: I,
) -> Vec<Hash>
where
    I: IntoIterator<Item = &'a Verified<AnyTx>>,
{
    try_add_one_height_with_transactions(sandbox, sandbox_state, txs).unwrap()
}

pub fn try_add_one_height_with_transactions<'a, I>(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    txs: I,
) -> Result<Vec<Hash>, String>
where
    I: IntoIterator<Item = &'a Verified<AnyTx>>,
{
    // sort transaction in order accordingly their hashes
    let absent_txs = sandbox.filter_present_transactions(txs);
    let mut tx_pool = BTreeMap::new();
    tx_pool.extend(absent_txs.into_iter().map(|tx| (tx.object_hash(), tx)));
    let raw_txs = tx_pool.values().cloned().collect::<Vec<_>>();

    trace!("=========================add_one_height_with_timeout started=========================");
    let initial_height = sandbox.current_epoch();
    // assert 1st round
    sandbox.assert_state(initial_height, Round(1));

    let mut hashes = Vec::with_capacity(raw_txs.len());
    for tx in &raw_txs {
        sandbox.recv(tx);
        hashes.push(tx.object_hash());
    }
    *sandbox_state.committed_transaction_hashes.borrow_mut() = hashes.clone();

    let n_validators = sandbox.validators().len();
    let new_height = initial_height.next();

    if n_validators == 1 {
        try_add_round_with_transactions(sandbox, sandbox_state, &hashes, true)?;
        let block = sandbox.last_block();
        assert_eq!(block.height, initial_height);
        assert_eq!(block.tx_count, hashes.len() as u32);
        assert_eq!(block.tx_hash, compute_txs_merkle_root(&hashes));
        sandbox.assert_state(new_height, Round(1));

        return Ok(hashes);
    }

    for _ in 0..n_validators {
        let propose = try_add_round_with_transactions(sandbox, sandbox_state, &hashes, false)?;
        let round = sandbox.current_round();
        if sandbox.is_leader() {
            // ok, we are leader
            trace!("ok, we are leader, round: {:?}", round);
            let propose = propose.unwrap();
            trace!("propose.hash: {:?}", propose.object_hash());
            trace!("sandbox.last_hash(): {:?}", sandbox.last_hash());
            *sandbox_state.accepted_propose_hash.borrow_mut() = propose.object_hash();

            for val_idx in 1..Sandbox::majority_count(n_validators) {
                let val_idx = ValidatorId(val_idx as u16);
                sandbox.recv(&sandbox.create_prevote(
                    val_idx,
                    initial_height,
                    round,
                    propose.object_hash(),
                    NOT_LOCKED,
                    sandbox.secret_key(val_idx),
                ));
            }

            sandbox.assert_lock(round, Some(propose.object_hash()));

            trace!("last_block: {:?}", sandbox.last_block());
            trace!(
                "last_block.object_hash(): {:?}",
                sandbox.last_block().object_hash()
            );

            let (state_hash, error_hash) = sandbox.compute_block_hashes(&raw_txs);
            let block = BlockBuilder::new(sandbox)
                .with_txs_hashes(&hashes)
                .with_state_hash(&state_hash)
                .with_error_hash(&error_hash)
                .build();

            trace!("new_block: {:?}", block);
            trace!("new_block.object_hash(): {:?}", block.object_hash());
            *sandbox_state.accepted_block_hash.borrow_mut() = block.object_hash();

            sandbox.broadcast(&sandbox.create_precommit(
                ValidatorId(0),
                initial_height,
                round,
                propose.object_hash(),
                block.object_hash(),
                sandbox.time().into(),
                sandbox.secret_key(ValidatorId(0)),
            ));
            sandbox.assert_lock(round, Some(propose.object_hash()));

            for val_idx in 1..Sandbox::majority_count(n_validators) {
                let val_idx = ValidatorId(val_idx as u16);
                sandbox.recv(&sandbox.create_precommit(
                    val_idx,
                    initial_height,
                    round,
                    propose.object_hash(),
                    block.object_hash(),
                    sandbox.time().into(),
                    sandbox.secret_key(val_idx),
                ));

                if val_idx.0 as usize != Sandbox::majority_count(n_validators) - 1 {
                    sandbox.assert_state(initial_height, round);
                }
            }

            sandbox.assert_state(new_height, Round(1));
            *sandbox_state.time_since_round_start.borrow_mut() = 0;
            sandbox.check_broadcast_status(new_height, block.object_hash());

            return Ok(hashes);
        }
    }

    Err("because at one of loops we should become a leader and return".into())
}

pub fn add_one_height_with_transactions_from_other_validator(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    txs: &[Verified<AnyTx>],
) -> Vec<Hash> {
    // sort transaction in order accordingly their hashes
    let mut tx_pool = BTreeMap::new();
    tx_pool.extend(txs.iter().map(|tx| (tx.object_hash(), tx.clone())));
    let raw_txs = tx_pool.values().cloned().collect::<Vec<_>>();

    trace!("=========================add_one_height_with_timeout started=========================");
    let initial_height = sandbox.current_epoch();
    // assert 1st round
    sandbox.assert_state(initial_height, Round(1));

    let mut hashes = Vec::with_capacity(raw_txs.len());
    for tx in &raw_txs {
        sandbox.recv(tx);
        hashes.push(tx.object_hash());
    }

    *sandbox_state.committed_transaction_hashes.borrow_mut() = hashes.clone();
    let n_validators = sandbox.validators().len();
    for _ in 0..n_validators {
        add_round_with_transactions(sandbox, sandbox_state, hashes.as_ref());
        let round = sandbox.current_round();
        if ValidatorId(1) == sandbox.leader(round) {
            sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));
            // ok, we are leader
            trace!("ok, validator 1 leader, round: {:?}", round);
            let propose = get_propose_with_transactions_for_validator(
                sandbox,
                hashes.as_ref(),
                ValidatorId(1),
            );
            trace!("propose.hash: {:?}", propose.object_hash());
            trace!("sandbox.last_hash(): {:?}", sandbox.last_hash());
            sandbox.recv(&propose);
            for val_idx in 0..Sandbox::majority_count(n_validators) {
                let val_idx = ValidatorId(val_idx as u16);
                sandbox.recv(&sandbox.create_prevote(
                    val_idx,
                    initial_height,
                    round,
                    propose.object_hash(),
                    NOT_LOCKED,
                    sandbox.secret_key(val_idx),
                ));
            }
            sandbox.assert_lock(round, Some(propose.object_hash()));

            trace!("last_block: {:?}", sandbox.last_block());
            let (state_hash, error_hash) = sandbox.compute_block_hashes(&raw_txs);
            let block = BlockBuilder::new(sandbox)
                .with_txs_hashes(&hashes)
                .with_state_hash(&state_hash)
                .with_error_hash(&error_hash)
                .build();
            trace!("new_block: {:?}", block);
            trace!("new_block.object_hash(): {:?}", block.object_hash());

            sandbox.assert_lock(round, Some(propose.object_hash()));
            sandbox.assert_state(initial_height, round);

            for val_idx in 0..Sandbox::majority_count(n_validators) {
                let val_idx = ValidatorId(val_idx as u16);
                sandbox.recv(&sandbox.create_precommit(
                    val_idx,
                    initial_height,
                    round,
                    propose.object_hash(),
                    block.object_hash(),
                    sandbox.time().into(),
                    sandbox.secret_key(val_idx),
                ));
            }

            let new_height = initial_height.next();
            sandbox.assert_state(new_height, Round(1));
            sandbox.check_broadcast_status(new_height, block.object_hash());

            *sandbox_state.time_since_round_start.borrow_mut() = 0;
            return hashes;
        }
    }

    unreachable!("because at one of loops we should become a leader and return");
}

fn get_propose_with_transactions(
    sandbox: &TimestampingSandbox,
    transactions: &[Hash],
) -> Verified<Propose> {
    get_propose_with_transactions_for_validator(sandbox, transactions, ValidatorId(0))
}

fn get_propose_with_transactions_for_validator(
    sandbox: &TimestampingSandbox,
    transactions: &[Hash],
    validator: ValidatorId,
) -> Verified<Propose> {
    trace!("sandbox.current_round: {:?}", sandbox.current_round());
    sandbox.create_propose(
        validator,
        sandbox.current_epoch(),
        sandbox.current_round(),
        sandbox.last_hash(),
        transactions.iter().cloned(),
        sandbox.secret_key(validator),
    )
}

fn try_check_and_broadcast_propose_and_prevote(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    transactions: &[Hash],
) -> Result<Option<Verified<Propose>>, String> {
    let millis_since_round_start = *sandbox_state.time_since_round_start.borrow();
    if millis_since_round_start > PROPOSE_TIMEOUT {
        return Ok(None);
    }
    let time_increment_millis = PROPOSE_TIMEOUT - millis_since_round_start + 1;

    trace!(
        "time elapsed in current round: {:?}",
        millis_since_round_start
    );
    trace!("going to add {:?} millis", time_increment_millis);
    sandbox.add_time(Duration::from_millis(time_increment_millis));
    *sandbox_state.time_since_round_start.borrow_mut() =
        millis_since_round_start + time_increment_millis;
    trace!("sandbox_time after adding: {:?}", sandbox.time());

    // ok, we are leader
    let propose = get_propose_with_transactions(sandbox, transactions);

    trace!("broadcasting propose: {:?}", propose);
    trace!(
        "broadcasting propose with hash: {:?}",
        propose.object_hash()
    );
    trace!(
        "broadcasting propose with round: {:?}",
        propose.payload().round
    );
    trace!("sandbox.current_round: {:?}", sandbox.current_round());
    sandbox.try_broadcast(&propose)?;

    sandbox.broadcast(&sandbox.create_prevote(
        ValidatorId(0),
        sandbox.current_epoch(),
        sandbox.current_round(),
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    ));
    Ok(Some(propose))
}

/// Idea of method is sandbox to receive correct propose from certain validator
/// with certain transactions
pub fn receive_valid_propose_with_transactions(
    sandbox: &TimestampingSandbox,
    transactions: &[Hash],
) -> Verified<Propose> {
    let propose = sandbox.create_propose(
        sandbox.current_leader(),
        sandbox.current_epoch(),
        sandbox.current_round(),
        sandbox.last_hash(),
        transactions.iter().cloned(),
        sandbox.secret_key(sandbox.current_leader()),
    );
    sandbox.recv(&propose);
    propose
}

pub fn make_request_propose_from_precommit(
    sandbox: &TimestampingSandbox,
    precommit: &Precommit,
) -> Verified<ProposeRequest> {
    Sandbox::create_propose_request(
        sandbox.public_key(ValidatorId(0)),
        sandbox.public_key(precommit.validator),
        precommit.epoch,
        precommit.propose_hash,
        sandbox.secret_key(ValidatorId(0)),
    )
}

pub fn make_request_prevote_from_precommit(
    sandbox: &TimestampingSandbox,
    precommit: &Precommit,
) -> Verified<PrevotesRequest> {
    let validators = BitVec::from_elem(sandbox.validators().len(), false);
    Sandbox::create_prevote_request(
        sandbox.public_key(ValidatorId(0)),
        sandbox.public_key(precommit.validator),
        precommit.epoch,
        precommit.round,
        precommit.propose_hash,
        validators,
        sandbox.secret_key(ValidatorId(0)),
    )
}

/// idea of the method is to return valid Prevote using provided Propose.
/// locked round is set to 0; may be need to take it from somewhere (from sandbox?)
pub fn make_prevote_from_propose(
    sandbox: &TimestampingSandbox,
    propose: &Verified<Propose>,
) -> Verified<Prevote> {
    sandbox.create_prevote(
        ValidatorId(0),
        propose.payload().epoch,
        propose.payload().round,
        propose.object_hash(),
        NOT_LOCKED,
        sandbox.secret_key(ValidatorId(0)),
    )
}
