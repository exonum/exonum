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

/// purpose of this module is to keep functions with reusable code used for sandbox tests
use bit_vec::BitVec;

use std::{cell::RefCell, collections::BTreeMap, time::Duration};

use super::{
    sandbox::Sandbox, timestamping::{TimestampTx, TimestampingTxGenerator},
};
use blockchain::{Block, SCHEMA_MAJOR_VERSION};
use crypto::{CryptoHash, Hash, HASH_SIZE};
use helpers::{Height, Milliseconds, Round, ValidatorId};
use messages::{
    Message, Precommit, Prevote, PrevotesRequest, Propose, ProposeRequest, RawTransaction,
};
use storage::Database;

pub type TimestampingSandbox = Sandbox;

pub const HEIGHT_ZERO: Height = Height(0);
pub const HEIGHT_ONE: Height = Height(1);
pub const HEIGHT_TWO: Height = Height(2);
pub const HEIGHT_THREE: Height = Height(3);
pub const HEIGHT_FOUR: Height = Height(4);
pub const LOCK_ZERO: Round = Round(0);
pub const LOCK_ONE: Round = Round(1);
pub const LOCK_TWO: Round = Round(2);
pub const ROUND_ONE: Round = Round(1);
pub const ROUND_TWO: Round = Round(2);
pub const ROUND_THREE: Round = Round(3);
pub const ROUND_FOUR: Round = Round(4);
pub const VALIDATOR_0: ValidatorId = ValidatorId(0);
pub const VALIDATOR_1: ValidatorId = ValidatorId(1);
pub const VALIDATOR_2: ValidatorId = ValidatorId(2);
pub const VALIDATOR_3: ValidatorId = ValidatorId(3);
pub const INCORRECT_VALIDATOR_ID: ValidatorId = ValidatorId(64_999);
pub const PROPOSE_TIMEOUT: Milliseconds = 200;

// Idea of ProposeBuilder is to implement Builder pattern in order to get Block with
// default data from sandbox and, possibly, update few fields with custom data.
pub struct BlockBuilder<'a> {
    proposer_id: Option<ValidatorId>,
    height: Option<Height>,
    duration_since_sandbox_time: Option<Milliseconds>,
    prev_hash: Option<Hash>,
    tx_hash: Option<Hash>,
    state_hash: Option<Hash>,
    tx_count: Option<u32>,

    sandbox: &'a TimestampingSandbox,
}

impl<'a> BlockBuilder<'a> {
    pub fn new(sandbox: &'a TimestampingSandbox) -> Self {
        BlockBuilder {
            proposer_id: None,
            height: None,
            duration_since_sandbox_time: None,
            prev_hash: None,
            tx_hash: None,
            state_hash: None,
            tx_count: None,

            sandbox,
        }
    }

    pub fn with_proposer_id(mut self, proposer_id: ValidatorId) -> Self {
        self.proposer_id = Some(proposer_id);
        self
    }

    pub fn with_height(mut self, height: Height) -> Self {
        self.height = Some(height);
        self
    }

    pub fn with_duration_since_sandbox_time(
        mut self,
        duration_since_sandbox_time: Milliseconds,
    ) -> Self {
        self.duration_since_sandbox_time = Some(duration_since_sandbox_time);
        self
    }

    pub fn with_prev_hash(mut self, prev_hash: &'a Hash) -> Self {
        self.prev_hash = Some(*prev_hash);
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

    pub fn build(&self) -> Block {
        Block::new(
            SCHEMA_MAJOR_VERSION,
            self.proposer_id
                .unwrap_or_else(|| self.sandbox.current_leader()),
            self.height.unwrap_or_else(|| self.sandbox.current_height()),
            self.tx_count.unwrap_or(0),
            &self.prev_hash.unwrap_or_else(|| self.sandbox.last_hash()),
            &self.tx_hash.unwrap_or_else(Hash::zero),
            &self.state_hash
                .unwrap_or_else(|| self.sandbox.last_state_hash()),
        )
    }
}

// Idea of ProposeBuilder is to implement Builder pattern in order to get Propose with
// default data from sandbox and, possibly, update few fields with custom data.
pub struct ProposeBuilder<'a> {
    validator_id: Option<ValidatorId>,
    height: Option<Height>,
    round: Option<Round>,
    duration_since_sandbox_time: Option<Milliseconds>,
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
            duration_since_sandbox_time: None,
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

    pub fn with_duration_since_sandbox_time(
        mut self,
        duration_since_sandbox_time: Milliseconds,
    ) -> Self {
        self.duration_since_sandbox_time = Some(duration_since_sandbox_time);
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

    pub fn build(&self) -> Propose {
        Propose::new(
            self.validator_id
                .unwrap_or_else(|| self.sandbox.current_leader()),
            self.height.unwrap_or_else(|| self.sandbox.current_height()),
            self.round.unwrap_or_else(|| self.sandbox.current_round()),
            self.prev_hash.unwrap_or(&self.sandbox.last_hash()),
            self.tx_hashes.unwrap_or(&[]),
            self.sandbox.s(self.validator_id
                .unwrap_or_else(|| self.sandbox.current_leader())),
        )
    }
}

pub struct SandboxState {
    pub accepted_propose_hash: RefCell<Hash>,
    pub accepted_block_hash: RefCell<Hash>,
    pub committed_transaction_hashes: RefCell<Vec<Hash>>,
    pub time_millis_since_round_start: RefCell<Milliseconds>,
}

impl SandboxState {
    pub fn new() -> Self {
        SandboxState::default()
    }
}

impl Default for SandboxState {
    fn default() -> Self {
        SandboxState {
            accepted_block_hash: RefCell::new(empty_hash()),
            accepted_propose_hash: RefCell::new(empty_hash()),
            committed_transaction_hashes: RefCell::new(Vec::new()),
            time_millis_since_round_start: RefCell::new(0),
        }
    }
}

/// Returns valid Hash object filled with zeros.
pub fn empty_hash() -> Hash {
    Hash::from_slice(&[0; HASH_SIZE]).unwrap()
}

pub fn compute_txs_merkle_root(txs: &[Hash]) -> Hash {
    use storage::{MemoryDB, ProofListIndex};

    let mut fork = MemoryDB::new().fork();
    let mut hashes = ProofListIndex::new("name", &mut fork);
    hashes.extend(txs.iter().cloned());
    hashes.merkle_root()
}

pub fn add_round_with_transactions(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    transactions: &[Hash],
) -> Option<Propose> {
    try_add_round_with_transactions(sandbox, sandbox_state, transactions).unwrap()
}

pub fn try_add_round_with_transactions(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    transactions: &[Hash],
) -> Result<Option<Propose>, String> {
    let mut res = None;
    let round_timeout = sandbox.round_timeout(); //use local var to save long code call

    trace!("-------------------------add_round_with_transactions started-------------------------");
    trace!("round before: {:?}", sandbox.current_round());
    trace!("sandbox_time: {:?}", sandbox.time());
    trace!("is_leader before time adding: {:?}", sandbox.is_leader());

    if sandbox.is_leader() {
        res = check_and_broadcast_propose_and_prevote(sandbox, sandbox_state, transactions);
    }

    // how much time left till next round_timeout
    let time_till_next_round: Milliseconds =
        round_timeout - *sandbox_state.time_millis_since_round_start.borrow() % round_timeout;

    trace!("going to add {:?} millis", time_till_next_round);
    sandbox.add_time(Duration::from_millis(time_till_next_round)); //here next round begins
    trace!("sandbox_time after adding: {:?}", sandbox.time());
    trace!("round after: {:?}", sandbox.current_round());
    trace!("sandbox.current_round: {:?}", sandbox.current_round());

    trace!("is_leader after time adding: {:?}", sandbox.is_leader());
    {
        *sandbox_state.time_millis_since_round_start.borrow_mut() = 0;
    }

    if sandbox.is_leader() {
        res = try_check_and_broadcast_propose_and_prevote(sandbox, sandbox_state, transactions)?;
    }
    Ok(res)
}

pub fn gen_timestamping_tx() -> TimestampTx {
    let mut tx_gen = TimestampingTxGenerator::new(64);
    tx_gen.next().unwrap()
}

pub fn add_one_height(sandbox: &TimestampingSandbox, sandbox_state: &SandboxState) {
    // gen some tx
    let tx = gen_timestamping_tx();
    add_one_height_with_transactions(sandbox, sandbox_state, &[tx.raw().clone()]);
}

pub fn add_one_height_with_transactions<'a, I>(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    txs: I,
) -> Vec<Hash>
where
    I: IntoIterator<Item = &'a RawTransaction>,
{
    try_add_one_height_with_transactions(sandbox, sandbox_state, txs).unwrap()
}

pub fn try_add_one_height(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
) -> Result<(), String> {
    // gen some tx
    let tx = gen_timestamping_tx();
    let result = try_add_one_height_with_transactions(sandbox, sandbox_state, &[tx.raw().clone()]);
    match result {
        Ok(_) => Ok(()),
        Err(msg) => Err(msg),
    }
}

pub fn try_add_one_height_with_transactions<'a, I>(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    txs: I,
) -> Result<Vec<Hash>, String>
where
    I: IntoIterator<Item = &'a RawTransaction>,
{
    // sort transaction in order accordingly their hashes
    let txs = sandbox.filter_present_transactions(txs);
    let mut tx_pool = BTreeMap::new();
    tx_pool.extend(txs.into_iter().map(|tx| (tx.hash(), tx.clone())));
    let raw_txs = tx_pool.values().cloned().collect::<Vec<_>>();
    let txs: &[RawTransaction] = raw_txs.as_ref();

    trace!("=========================add_one_height_with_timeout started=========================");
    let initial_height = sandbox.current_height();
    // assert 1st round
    sandbox.assert_state(initial_height, ROUND_ONE);

    let hashes = {
        let mut hashes = Vec::new();
        for tx in txs.iter() {
            sandbox.recv(tx);
            hashes.push(tx.hash());
        }
        hashes
    };
    {
        *sandbox_state.committed_transaction_hashes.borrow_mut() = hashes.clone();
    }
    let mut propose: Option<Propose>;

    let n_validators = sandbox.n_validators();
    for _ in 0..n_validators {
        propose = try_add_round_with_transactions(sandbox, sandbox_state, hashes.as_ref())?;
        let round = sandbox.current_round();
        if sandbox.is_leader() {
            // ok, we are leader
            trace!("ok, we are leader, round: {:?}", round);
            let propose = propose.unwrap();
            trace!("propose.hash: {:?}", propose.hash());
            trace!("sandbox.last_hash(): {:?}", sandbox.last_hash());
            {
                *sandbox_state.accepted_propose_hash.borrow_mut() = propose.hash();
            }

            for val_idx in 1..sandbox.majority_count(n_validators) {
                let val_idx = ValidatorId(val_idx as u16);
                sandbox.recv(&Prevote::new(
                    val_idx,
                    initial_height,
                    round,
                    &propose.hash(),
                    LOCK_ZERO,
                    sandbox.s(val_idx),
                ));
            }
            sandbox.assert_lock(round, Some(propose.hash()));

            trace!("last_block: {:?}", sandbox.last_block());
            trace!("last_block.hash(): {:?}", sandbox.last_block().hash());

            let state_hash = sandbox.compute_state_hash(&raw_txs);
            let block = BlockBuilder::new(sandbox)
                .with_txs_hashes(&hashes)
                .with_state_hash(&state_hash)
                .build();

            trace!("new_block: {:?}", block);
            trace!("new_block.hash(): {:?}", block.hash());
            {
                *sandbox_state.accepted_block_hash.borrow_mut() = block.hash();
            }

            sandbox.broadcast(&Precommit::new(
                VALIDATOR_0,
                initial_height,
                round,
                &propose.hash(),
                &block.hash(),
                sandbox.time().into(),
                sandbox.s(VALIDATOR_0),
            ));
            sandbox.assert_lock(round, Some(propose.hash()));

            for val_idx in 1..sandbox.majority_count(n_validators) {
                let val_idx = ValidatorId(val_idx as u16);
                sandbox.recv(&Precommit::new(
                    val_idx,
                    initial_height,
                    round,
                    &propose.hash(),
                    &block.hash(),
                    sandbox.time().into(),
                    sandbox.s(val_idx),
                ));

                if val_idx.0 as usize != sandbox.majority_count(n_validators) - 1 {
                    sandbox.assert_state(initial_height, round);
                }
            }

            let new_height = initial_height.next();
            sandbox.assert_state(new_height, ROUND_ONE);
            {
                *sandbox_state.time_millis_since_round_start.borrow_mut() = 0;
            }
            sandbox.check_broadcast_status(new_height, &block.hash());

            return Ok(hashes);
        }
    }

    Err("because at one of loops we should become a leader and return".into())
}

pub fn add_one_height_with_transactions_from_other_validator(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    txs: &[RawTransaction],
) -> Vec<Hash> {
    // sort transaction in order accordingly their hashes
    let mut tx_pool = BTreeMap::new();
    tx_pool.extend(txs.into_iter().map(|tx| (tx.hash(), tx.clone())));
    let raw_txs = tx_pool.values().cloned().collect::<Vec<_>>();
    let txs: &[RawTransaction] = raw_txs.as_ref();

    trace!("=========================add_one_height_with_timeout started=========================");
    let initial_height = sandbox.current_height();
    // assert 1st round
    sandbox.assert_state(initial_height, ROUND_ONE);

    let hashes = {
        let mut hashes = Vec::new();
        for tx in txs.iter() {
            sandbox.recv(tx);
            hashes.push(tx.hash());
        }
        hashes
    };

    {
        *sandbox_state.committed_transaction_hashes.borrow_mut() = hashes.clone();
    }
    let n_validators = sandbox.n_validators();
    for _ in 0..n_validators {
        //        add_round_with_transactions(&sandbox, &[tx.hash()]);
        add_round_with_transactions(sandbox, sandbox_state, hashes.as_ref());
        let round = sandbox.current_round();
        if VALIDATOR_1 == sandbox.leader(round) {
            sandbox.add_time(Duration::from_millis(PROPOSE_TIMEOUT));
            // ok, we are leader
            trace!("ok, validator 1 leader, round: {:?}", round);
            let propose =
                get_propose_with_transactions_for_validator(sandbox, hashes.as_ref(), VALIDATOR_1);
            trace!("propose.hash: {:?}", propose.hash());
            trace!("sandbox.last_hash(): {:?}", sandbox.last_hash());
            sandbox.recv(&propose);
            for val_idx in 0..sandbox.majority_count(n_validators) {
                let val_idx = ValidatorId(val_idx as u16);
                sandbox.recv(&Prevote::new(
                    val_idx,
                    initial_height,
                    round,
                    &propose.hash(),
                    LOCK_ZERO,
                    sandbox.s(val_idx),
                ));
            }
            sandbox.assert_lock(round, Some(propose.hash()));

            trace!("last_block: {:?}", sandbox.last_block());
            let state_hash = sandbox.compute_state_hash(&raw_txs);
            let block = BlockBuilder::new(sandbox)
                .with_txs_hashes(&hashes)
                .with_state_hash(&state_hash)
                .build();
            trace!("new_block: {:?}", block);
            trace!("new_block.hash(): {:?}", block.hash());

            sandbox.assert_lock(round, Some(propose.hash()));
            sandbox.assert_state(initial_height, round);

            for val_idx in 0..sandbox.majority_count(n_validators) {
                let val_idx = ValidatorId(val_idx as u16);
                sandbox.recv(&Precommit::new(
                    val_idx,
                    initial_height,
                    round,
                    &propose.hash(),
                    &block.hash(),
                    sandbox.time().into(),
                    sandbox.s(val_idx),
                ));
            }

            let new_height = initial_height.next();
            sandbox.assert_state(new_height, ROUND_ONE);
            sandbox.check_broadcast_status(new_height, &block.hash());

            {
                *sandbox_state.time_millis_since_round_start.borrow_mut() = 0;
            }
            return hashes;
        }
    }

    unreachable!("because at one of loops we should become a leader and return");
}

fn get_propose_with_transactions(sandbox: &TimestampingSandbox, transactions: &[Hash]) -> Propose {
    get_propose_with_transactions_for_validator(sandbox, transactions, VALIDATOR_0)
}

fn get_propose_with_transactions_for_validator(
    sandbox: &TimestampingSandbox,
    transactions: &[Hash],
    validator: ValidatorId,
) -> Propose {
    trace!("sandbox.current_round: {:?}", sandbox.current_round());
    Propose::new(
        validator,
        sandbox.current_height(),
        sandbox.current_round(),
        &sandbox.last_hash(),
        transactions,
        sandbox.s(validator),
    )
}

/// assumptions:
/// - that we come in this function with leader state
/// - in current round `propose_timeout` is not triggered yet
/// - `propose_timeout` < `round_timeout`
fn check_and_broadcast_propose_and_prevote(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    transactions: &[Hash],
) -> Option<Propose> {
    try_check_and_broadcast_propose_and_prevote(sandbox, sandbox_state, transactions).unwrap()
}

fn try_check_and_broadcast_propose_and_prevote(
    sandbox: &TimestampingSandbox,
    sandbox_state: &SandboxState,
    transactions: &[Hash],
) -> Result<Option<Propose>, String> {
    if *sandbox_state.time_millis_since_round_start.borrow() > PROPOSE_TIMEOUT {
        return Ok(None);
    }

    let time_millis_since_round_start_copy =
        { *sandbox_state.time_millis_since_round_start.borrow() };
    let time_increment_millis = PROPOSE_TIMEOUT - time_millis_since_round_start_copy + 1;

    trace!(
        "time elapsed in current round: {:?}",
        sandbox_state.time_millis_since_round_start
    );
    trace!("going to add {:?} millis", time_increment_millis);
    sandbox.add_time(Duration::from_millis(time_increment_millis));
    {
        *sandbox_state.time_millis_since_round_start.borrow_mut() =
            time_millis_since_round_start_copy + time_increment_millis;
    }
    trace!("sandbox_time after adding: {:?}", sandbox.time());

    // ok, we are leader
    let propose = get_propose_with_transactions(sandbox, transactions);

    trace!("broadcasting propose: {:?}", propose);
    trace!("broadcasting propose with hash: {:?}", propose.hash());
    trace!("broadcasting propose with round: {:?}", propose.round());
    trace!("sandbox.current_round: {:?}", sandbox.current_round());
    sandbox.try_broadcast(&propose)?;

    sandbox.broadcast(&Prevote::new(
        VALIDATOR_0,
        sandbox.current_height(),
        sandbox.current_round(),
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    ));
    Ok(Some(propose.clone()))
}

/// Idea of method is sandbox to receive correct propose from certain validator
/// with certain transactions
pub fn receive_valid_propose_with_transactions(
    sandbox: &TimestampingSandbox,
    transactions: &[Hash],
) -> Propose {
    let propose = Propose::new(
        sandbox.current_leader(),
        sandbox.current_height(),
        sandbox.current_round(),
        &sandbox.last_hash(),
        transactions,
        sandbox.s(sandbox.current_leader()),
    );
    sandbox.recv(&propose);
    propose.clone()
}

pub fn make_request_propose_from_precommit(
    sandbox: &TimestampingSandbox,
    precommit: &Precommit,
) -> ProposeRequest {
    ProposeRequest::new(
        &sandbox.p(VALIDATOR_0),
        &sandbox.p(precommit.validator()),
        precommit.height(),
        precommit.propose_hash(),
        sandbox.s(VALIDATOR_0),
    )
}

pub fn make_request_prevote_from_precommit(
    sandbox: &TimestampingSandbox,
    precommit: &Precommit,
) -> PrevotesRequest {
    let validators = BitVec::from_elem(sandbox.n_validators(), false);
    PrevotesRequest::new(
        &sandbox.p(VALIDATOR_0),
        &sandbox.p(precommit.validator()),
        precommit.height(),
        precommit.round(),
        precommit.propose_hash(),
        validators,
        sandbox.s(VALIDATOR_0),
    )
}

/// idea of the method is to return valid Prevote using provided Propose.
/// locked round is set to 0; may be need to take it from somewhere (from sandbox?)
pub fn make_prevote_from_propose(sandbox: &TimestampingSandbox, propose: &Propose) -> Prevote {
    Prevote::new(
        VALIDATOR_0,
        propose.height(),
        propose.round(),
        &propose.hash(),
        LOCK_ZERO,
        sandbox.s(VALIDATOR_0),
    )
}
