/// purpose of this module is to keep functions with reusable code used for sandbox tests

use std::time::Duration;
use std::cell::RefCell;
use std::collections::BTreeMap;

use exonum::messages::{RawTransaction, Message, Propose, Prevote, Precommit, RequestPropose,
                       RequestPrevotes, BitVec};
use exonum::blockchain::{Block, SCHEMA_MAJOR_VERSION};
use exonum::crypto::{Hash, HASH_SIZE, hash};
use exonum::events::Milliseconds;
use exonum::node::ValidatorId;

use sandbox::Sandbox;
use timestamping::{TimestampTx, TimestampingTxGenerator};

pub type TimestampingSandbox = Sandbox;

pub const HEIGHT_ZERO: u64 = 0;
pub const HEIGHT_ONE: u64 = 1;
pub const HEIGHT_TWO: u64 = 2;
pub const HEIGHT_THREE: u64 = 3;
pub const HEIGHT_FOUR: u64 = 4;
pub const LOCK_ZERO: u32 = 0;
pub const LOCK_ONE: u32 = 1;
pub const LOCK_TWO: u32 = 2;
pub const ROUND_ONE: u32 = 1;
pub const ROUND_TWO: u32 = 2;
pub const ROUND_THREE: u32 = 3;
pub const ROUND_FOUR: u32 = 4;
pub const ROUND_FIVE: u32 = 5;
pub const VALIDATOR_0: u32 = 0;
pub const VALIDATOR_1: u32 = 1;
pub const VALIDATOR_2: u32 = 2;
pub const VALIDATOR_3: u32 = 3;
pub const INCORRECT_VALIDATOR_ID: u32 = 999_999;

// Idea of ProposeBuilder is to implement Builder pattern in order to get Block with
// default data from sandbox and, possibly, update few fields with custom data.
pub struct BlockBuilder<'a> {
    network_id: Option<u8>,
    proposer_id: Option<u32>,
    height: Option<u64>,
    duration_since_sandbox_time: Option<Milliseconds>,
    prev_hash: Option<Hash>,
    tx_hash: Option<Hash>,
    state_hash: Option<Hash>,

    sandbox: &'a TimestampingSandbox,
}

impl<'a> BlockBuilder<'a> {
    pub fn new(sandbox: &'a TimestampingSandbox) -> Self {
        BlockBuilder {
            network_id: None,
            proposer_id: None,
            height: None,
            duration_since_sandbox_time: None,
            prev_hash: None,
            tx_hash: None,
            state_hash: None,

            sandbox: sandbox,
        }
    }

    pub fn with_network_id(mut self, network_id: u8) -> Self {
        self.network_id = Some(network_id);
        self
    }

    pub fn with_proposer_id(mut self, proposer_id: u32) -> Self {
        self.proposer_id = Some(proposer_id);
        self
    }

    pub fn with_height(mut self, height: u64) -> Self {
        self.height = Some(height);
        self
    }

    pub fn with_duration_since_sandbox_time(mut self,
                                            duration_since_sandbox_time: Milliseconds)
                                            -> Self {
        self.duration_since_sandbox_time = Some(duration_since_sandbox_time);
        self
    }

    pub fn with_prev_hash(mut self, prev_hash: &'a Hash) -> Self {
        self.prev_hash = Some(*prev_hash);
        self
    }

    pub fn with_tx_hash(mut self, individual_transaction_hash: &'a Hash) -> Self {
        // root of merkle table, containing this single transaction
        // exonum::storage::merkle_table
        // see how hash(&self) changed in exonum::storage::fields::StorageValue for Hash,
        // it's _hash(self.as_ref())_ as of now instead of _*self_ as it used to be
        let merkle_root = hash(individual_transaction_hash.as_ref());
        self.tx_hash = Some(merkle_root);
        self
    }

    pub fn with_txs_hashes(mut self, tx_hashes: &[Hash]) -> Self {
        // root of merkle table, containing this array of transactions
        let merkle_root = compute_txs_root_hash(tx_hashes);
        self.tx_hash = Some(merkle_root);
        self
    }

    pub fn with_state_hash(mut self, state_hash: &'a Hash) -> Self {
        self.state_hash = Some(*state_hash);
        self
    }

    pub fn build(&self) -> Block {
        Block::new(SCHEMA_MAJOR_VERSION,
                   self.network_id.unwrap_or_else(|| 0),
                   self.proposer_id
                       .unwrap_or_else(|| self.sandbox.current_leader()),
                   self.height.unwrap_or_else(|| self.sandbox.current_height()),
                   &self.prev_hash.unwrap_or_else(|| self.sandbox.last_hash()),
                   //   &[tx.hash(), tx2.hash()],
                   //   &[tx.hash()],
                   //   &[],
                   &self.tx_hash.unwrap_or_else(Hash::zero),
                   &self.state_hash
                        .unwrap_or_else(|| self.sandbox.last_state_hash()))
    }
}

// Idea of ProposeBuilder is to implement Builder pattern in order to get Propose with
// default data from sandbox and, possibly, update few fields with custom data.
pub struct ProposeBuilder<'a> {
    validator_id: Option<u32>,
    height: Option<u64>,
    round: Option<u32>,
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
            sandbox: sandbox,
        }
    }

    pub fn with_validator(mut self, validator_id: u32) -> Self {
        self.validator_id = Some(validator_id);
        self
    }

    pub fn with_height(mut self, height: u64) -> Self {
        self.height = Some(height);
        self
    }

    pub fn with_round(mut self, round: u32) -> Self {
        self.round = Some(round);
        self
    }

    pub fn with_duration_since_sandbox_time(mut self,
                                            duration_since_sandbox_time: Milliseconds)
                                            -> Self {
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
        Propose::new(self.validator_id
                         .unwrap_or_else(|| self.sandbox.current_leader()),
                     self.height.unwrap_or_else(|| self.sandbox.current_height()),
                     self.round.unwrap_or_else(|| self.sandbox.current_round()),
                     self.prev_hash.unwrap_or(&self.sandbox.last_hash()),
                     //   &[tx.hash(), tx2.hash()],
                     //   &[tx.hash()],
                     //   &[],
                     self.tx_hashes.unwrap_or(&[]),
                     self.sandbox
                         .s(self.validator_id
                                .unwrap_or_else(|| self.sandbox.current_leader()) as
                            usize))
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

/// just returns valid Hash object filled with zeros
pub fn empty_hash() -> Hash {
    Hash::from_slice(&[0; HASH_SIZE]).unwrap()
}

pub fn compute_txs_root_hash(txs: &[Hash]) -> Hash {
    // TODO use special function
    use exonum::storage::{MemoryDB, List, MerkleTable};

    let db = MemoryDB::new();
    let hashes: MerkleTable<MemoryDB, Hash> = MerkleTable::new(db);
    hashes.extend(txs.iter().cloned()).unwrap();
    hashes.root_hash().unwrap()
}

pub fn add_round_with_transactions(sandbox: &TimestampingSandbox,
                                   sandbox_state: &SandboxState,
                                   transactions: &[Hash])
                                   -> Option<Propose> {
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
        res = check_and_broadcast_propose_and_prevote(sandbox, sandbox_state, transactions);
    }
    res
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

pub fn add_one_height_with_transactions<'a, I>(sandbox: &TimestampingSandbox,
                                               sandbox_state: &SandboxState,
                                               txs: I)
    where I: IntoIterator<Item = &'a RawTransaction>
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
            sandbox.recv(tx.clone());
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
        propose = add_round_with_transactions(sandbox, sandbox_state, hashes.as_ref());
        let round: u32 = sandbox.current_round();
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
                sandbox.recv(Prevote::new(val_idx as u32,
                                          initial_height,
                                          round,
                                          &propose.hash(),
                                          LOCK_ZERO,
                                          sandbox.s(val_idx)));
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
            {
                *sandbox_state.accepted_block_hash.borrow_mut() = block.hash();
            }

            sandbox.broadcast(Precommit::new(VALIDATOR_0,
                                             initial_height,
                                             round,
                                             &propose.hash(),
                                             &block.hash(),
                                             sandbox.time(),
                                             sandbox.s(VALIDATOR_0 as usize)));
            sandbox.assert_lock(round, Some(propose.hash()));

            for val_idx in 1..sandbox.majority_count(n_validators) {
                sandbox.recv(Precommit::new(val_idx as u32,
                                            initial_height,
                                            round,
                                            &propose.hash(),
                                            &block.hash(),
                                            sandbox.time(),
                                            sandbox.s(val_idx)));

                if val_idx != sandbox.majority_count(n_validators) - 1 {
                    sandbox.assert_state(initial_height, round);
                }
            }

            let new_height = initial_height + 1;
            sandbox.assert_state(new_height, ROUND_ONE);
            {
                *sandbox_state.time_millis_since_round_start.borrow_mut() = 0;
            }
            sandbox.check_broadcast_status(new_height, &block.hash());

            return;
        }
    }

    unreachable!("because at one of loops we should become a leader and return");
}

pub fn add_one_height_with_transactions_from_other_validator(sandbox: &TimestampingSandbox,
                                                             sandbox_state: &SandboxState,
                                                             txs: &[RawTransaction]) {
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
            sandbox.recv(tx.clone());
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
        let round: u32 = sandbox.current_round();
        if VALIDATOR_1 == sandbox.leader(round) {
            sandbox.add_time(Duration::from_millis(sandbox.propose_timeout()));
            // ok, we are leader
            trace!("ok, validator 1 leader, round: {:?}", round);
            let propose =
                get_propose_with_transactions_for_validator(sandbox, hashes.as_ref(), VALIDATOR_1);
            trace!("propose.hash: {:?}", propose.hash());
            trace!("sandbox.last_hash(): {:?}", sandbox.last_hash());
            /* {
                *sandbox_state.accepted_propose_hash.borrow_mut() = propose.hash();
            }*/
            sandbox.recv(propose.clone());
            for val_idx in 0..sandbox.majority_count(n_validators) {
                sandbox.recv(Prevote::new(val_idx as u32,
                                          initial_height,
                                          round,
                                          &propose.hash(),
                                          LOCK_ZERO,
                                          sandbox.s(val_idx)));
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
                sandbox.recv(Precommit::new(val_idx as u32,
                                            initial_height,
                                            round,
                                            &propose.hash(),
                                            &block.hash(),
                                            sandbox.time(),
                                            sandbox.s(val_idx)));
            }

            let new_height = initial_height + 1;
            sandbox.assert_state(new_height, ROUND_ONE);
            sandbox.check_broadcast_status(new_height, &block.hash());

            {
                *sandbox_state.time_millis_since_round_start.borrow_mut() = 0;
            }
            return;
        }
    }

    unreachable!("because at one of loops we should become a leader and return");
}

fn get_propose_with_transactions(sandbox: &TimestampingSandbox, transactions: &[Hash]) -> Propose {

    get_propose_with_transactions_for_validator(sandbox, transactions, VALIDATOR_0)
}

fn get_propose_with_transactions_for_validator(sandbox: &TimestampingSandbox,
                                               transactions: &[Hash],
                                               validator: ValidatorId)
                                               -> Propose {
    trace!("sandbox.current_round: {:?}", sandbox.current_round());
    Propose::new(validator,
                 sandbox.current_height(),
                 sandbox.current_round(),
                 &sandbox.last_hash(),
                 //   &[tx.hash(), tx2.hash()],
                 //   &[tx.hash()],
                 //   &[],
                 transactions,
                 sandbox.s(validator as usize))
}

/// assumptions:
/// - that we come in this function with leader state
/// - in current round `propose_timeout` is not triggered yet
/// - `propose_timeout` < `round_timeout`
fn check_and_broadcast_propose_and_prevote(sandbox: &TimestampingSandbox,
                                           sandbox_state: &SandboxState,
                                           transactions: &[Hash])
                                           -> Option<Propose> {
    if *sandbox_state.time_millis_since_round_start.borrow() > sandbox.propose_timeout() {
        return None;
    }

    let time_millis_since_round_start_copy = {
        *sandbox_state.time_millis_since_round_start.borrow()
    };
    let time_increment_millis = sandbox.propose_timeout() - time_millis_since_round_start_copy + 1;

    trace!("time elapsed in current round: {:?}",
           sandbox_state.time_millis_since_round_start);
    //    trace!("going to add {:?} millis", round_timeout - 1);
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
    sandbox.broadcast(propose.clone());

    sandbox.broadcast(Prevote::new(VALIDATOR_0,
                                   sandbox.current_height(),
                                   sandbox.current_round(),
                                   &propose.hash(),
                                   LOCK_ZERO,
                                   sandbox.s(VALIDATOR_0 as usize)));
    Some(propose.clone())
}

/// Idea of method is sandbox to receive correct propose from certain validator
/// with certain transactions
pub fn receive_valid_propose_with_transactions(sandbox: &TimestampingSandbox,
                                               transactions: &[Hash])
                                               -> Propose {
    let propose = Propose::new(sandbox.current_leader(),
                               sandbox.current_height(),
                               sandbox.current_round(),
                               &sandbox.last_hash(),
                               //                               &[],
                               transactions,
                               sandbox.s(sandbox.current_leader() as usize));
    sandbox.recv(propose.clone());
    propose.clone()
}

pub fn make_request_propose_from_precommit(sandbox: &TimestampingSandbox,
                                           precommit: &Precommit)
                                           -> RequestPropose {
    RequestPropose::new(&sandbox.p(VALIDATOR_0 as usize),
                        &sandbox.p(precommit.validator() as usize),
                        precommit.height(),
                        precommit.propose_hash(),
                        sandbox.s(VALIDATOR_0 as usize))
}

pub fn make_request_prevote_from_precommit(sandbox: &TimestampingSandbox,
                                           precommit: &Precommit)
                                           -> RequestPrevotes {
    let validators = BitVec::from_elem(sandbox.n_validators(), false);
    //    validators.set(precommit.validator() as usize, true);
    RequestPrevotes::new(&sandbox.p(VALIDATOR_0 as usize),
                         &sandbox.p(precommit.validator() as usize),
                         precommit.height(),
                         precommit.round(),
                         precommit.propose_hash(),
                         validators,
                         sandbox.s(VALIDATOR_0 as usize))
}

pub fn last_block_hash(sandbox: &TimestampingSandbox) -> Hash {
    sandbox.last_hash()
}

/// idea of the method is to return valid Prevote using provided Propose.
/// locked round is set to 0; may be need to take it from somewhere (from sandbox?)
pub fn make_prevote_from_propose(sandbox: &TimestampingSandbox, propose: &Propose) -> Prevote {
    Prevote::new(VALIDATOR_0,
                 propose.height(),
                 propose.round(),
                 &propose.hash(),
                 LOCK_ZERO,
                 sandbox.s(VALIDATOR_0 as usize))
}
