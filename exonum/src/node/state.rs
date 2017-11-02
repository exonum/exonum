// Copyright 2017 The Exonum Team
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

//! State of the `NodeHandler`.

use serde_json::Value;
use bit_vec::BitVec;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::sync::{Arc, RwLock};
use std::net::SocketAddr;
use std::time::{SystemTime, Duration};

use messages::{Message, Propose, Prevote, Precommit, ConsensusMessage, Connect};
use crypto::{PublicKey, SecretKey, Hash};
use storage::{Patch, Snapshot};
use blockchain::{ValidatorKeys, ConsensusConfig, StoredConfiguration, Transaction,
                 TimeoutAdjusterConfig};
use helpers::{Height, Round, ValidatorId, Milliseconds};
use node::whitelist::Whitelist;
use node::timeout_adjuster::{TimeoutAdjuster, Constant, Dynamic, MovingAverage};

// TODO: move request timeouts into node configuration (ECR-171)

/// Timeout value for the `ProposeRequest` message.
pub const PROPOSE_REQUEST_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `TransactionsRequest` message.
pub const TRANSACTIONS_REQUEST_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `PrevotesRequest` message.
pub const PREVOTES_REQUEST_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `BlockRequest` message.
pub const BLOCK_REQUEST_TIMEOUT: Milliseconds = 100;

/// Transactions pool.
// TODO replace by persistent TxPool (ECR-171)
pub type TxPool = Arc<RwLock<BTreeMap<Hash, Box<Transaction>>>>;
// TODO: reduce copying of Hash (ECR-171)

/// State of the `NodeHandler`.
#[derive(Debug)]
pub struct State {
    validator_state: Option<ValidatorState>,
    our_connect_message: Connect,

    consensus_public_key: PublicKey,
    consensus_secret_key: SecretKey,
    service_public_key: PublicKey,
    service_secret_key: SecretKey,

    config: StoredConfiguration,
    whitelist: Whitelist,
    tx_pool_capacity: usize,

    peers: HashMap<PublicKey, Connect>,
    connections: HashMap<SocketAddr, PublicKey>,
    height_start_time: SystemTime,
    height: Height,

    round: Round,
    locked_round: Round,
    locked_propose: Option<Hash>,
    last_hash: Hash,

    // messages
    proposes: HashMap<Hash, ProposeState>,
    blocks: HashMap<Hash, BlockState>,
    prevotes: HashMap<(Round, Hash), Votes<Prevote>>,
    precommits: HashMap<(Round, Hash), Votes<Precommit>>,

    transactions: TxPool,

    queued: Vec<ConsensusMessage>,

    unknown_txs: HashMap<Hash, Vec<Hash>>,
    unknown_proposes_with_precommits: HashMap<Hash, Vec<(Round, Hash)>>,

    // Our requests state.
    requests: HashMap<RequestData, RequestState>,

    // maximum of node height in consensus messages
    nodes_max_height: BTreeMap<PublicKey, Height>,

    timeout_adjuster: Box<TimeoutAdjuster>,
    propose_timeout: Milliseconds,
}

/// State of a validator-node.
#[derive(Debug, Clone)]
pub struct ValidatorState {
    id: ValidatorId,
    our_prevotes: HashMap<Round, Prevote>,
    our_precommits: HashMap<Round, Precommit>,
}

/// `RequestData` represents a request for some data to other nodes. Each enum variant will be
/// translated to the corresponding request-message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestData {
    /// Represents `ProposeRequest` message.
    Propose(Hash),
    /// Represents `TransactionsRequest` message.
    Transactions(Hash),
    /// Represents `PrevotesRequest` message.
    Prevotes(Round, Hash),
    /// Represents `BlockRequest` message.
    Block(Height),
}

#[derive(Debug)]
struct RequestState {
    // Number of attempts made.
    retries: u16,
    // Nodes that have the required information.
    known_nodes: HashSet<PublicKey>,
}

/// `ProposeState` represents the state of some propose and is used for tracking of unknown
#[derive(Debug)]
/// transactions.
pub struct ProposeState {
    propose: Propose,
    unknown_txs: HashSet<Hash>,
    block_hash: Option<Hash>,
}

/// State of a block.
#[derive(Clone, Debug)]
pub struct BlockState {
    hash: Hash,
    // Changes that should be made for block committing.
    patch: Patch,
    txs: Vec<Hash>,
    proposer_id: ValidatorId,
}

/// `VoteMessage` trait represents voting messages such as `Precommit` and `Prevote`.
pub trait VoteMessage: Message + Clone {
    /// Return validator if of the message.
    fn validator(&self) -> ValidatorId;
}

impl VoteMessage for Precommit {
    fn validator(&self) -> ValidatorId {
        self.validator()
    }
}

impl VoteMessage for Prevote {
    fn validator(&self) -> ValidatorId {
        self.validator()
    }
}

/// Contains voting messages alongside with there validator ids.
#[derive(Debug)]
pub struct Votes<T: VoteMessage> {
    messages: Vec<T>,
    validators: BitVec,
    count: usize,
}

impl ValidatorState {
    /// Creates new `ValidatorState` with given validator id.
    pub fn new(id: ValidatorId) -> Self {
        ValidatorState {
            id: id,
            our_precommits: HashMap::new(),
            our_prevotes: HashMap::new(),
        }
    }

    /// Returns validator id.
    pub fn id(&self) -> ValidatorId {
        self.id
    }

    /// Sets new validator id.
    pub fn set_validator_id(&mut self, id: ValidatorId) {
        self.id = id;
    }

    /// Checks if the node has pre-vote for the specified round.
    pub fn have_prevote(&self, round: Round) -> bool {
        self.our_prevotes.get(&round).is_some()
    }

    /// Clears pre-commits and pre-votes.
    pub fn clear(&mut self) {
        self.our_precommits.clear();
        self.our_prevotes.clear();
    }
}

impl<T> Votes<T>
where
    T: VoteMessage,
{
    /// Creates a new `Votes` instance with a specified validators number.
    pub fn new(validators_len: usize) -> Votes<T> {
        Votes {
            messages: Vec::new(),
            validators: BitVec::from_elem(validators_len, false),
            count: 0,
        }
    }

    /// Inserts a new message if it hasn't been inserted yet.
    pub fn insert(&mut self, message: &T) {
        let voter: usize = message.validator().into();
        if !self.validators[voter] {
            self.count += 1;
            self.validators.set(voter, true);
            self.messages.push(message.clone());
        }
    }

    /// Returns validators.
    pub fn validators(&self) -> &BitVec {
        &self.validators
    }

    /// Returns number of contained messages.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Returns messages.
    pub fn messages(&self) -> &Vec<T> {
        &self.messages
    }
}

impl RequestData {
    /// Returns timeout value of the data request.
    pub fn timeout(&self) -> Duration {
        #![cfg_attr(feature="cargo-clippy", allow(match_same_arms))]
        let ms = match *self {
            RequestData::Propose(..) => PROPOSE_REQUEST_TIMEOUT,
            RequestData::Transactions(..) => TRANSACTIONS_REQUEST_TIMEOUT,
            RequestData::Prevotes(..) => PREVOTES_REQUEST_TIMEOUT,
            RequestData::Block(..) => BLOCK_REQUEST_TIMEOUT,
        };
        Duration::from_millis(ms)
    }
}

impl RequestState {
    fn new() -> Self {
        RequestState {
            retries: 0,
            known_nodes: HashSet::new(),
        }
    }

    fn insert(&mut self, peer: PublicKey) {
        self.known_nodes.insert(peer);
    }

    fn remove(&mut self, peer: &PublicKey) {
        self.retries += 1;
        self.known_nodes.remove(peer);
    }

    fn is_empty(&self) -> bool {
        self.known_nodes.is_empty()
    }

    fn peek(&self) -> Option<PublicKey> {
        self.known_nodes.iter().next().cloned()
    }
}

impl ProposeState {
    /// Returns hash of the propose.
    pub fn hash(&self) -> Hash {
        self.propose.hash()
    }

    /// Returns block hash propose was executed.
    pub fn block_hash(&self) -> Option<Hash> {
        self.block_hash
    }

    /// Set block hash on propose execute.
    pub fn set_block_hash(&mut self, block_hash: Hash) {
        self.block_hash = Some(block_hash)
    }

    /// Returns propose-message.
    pub fn message(&self) -> &Propose {
        &self.propose
    }

    /// Returns unknown transactions of the propose.
    pub fn unknown_txs(&self) -> &HashSet<Hash> {
        &self.unknown_txs
    }

    /// Returns `true` if there are unknown transactions in the propose.
    pub fn has_unknown_txs(&self) -> bool {
        !self.unknown_txs.is_empty()
    }
}

impl BlockState {
    /// Creates a new `BlockState` instance with the given parameters.
    pub fn new(hash: Hash, patch: Patch, txs: Vec<Hash>, proposer_id: ValidatorId) -> Self {
        BlockState {
            hash,
            patch,
            txs,
            proposer_id,
        }
    }

    /// Returns hash of the block.
    pub fn hash(&self) -> Hash {
        self.hash
    }

    /// Returns the changes that should be made for block committing.
    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    /// Returns block's transactions.
    pub fn txs(&self) -> &Vec<Hash> {
        &self.txs
    }

    /// Returns id of the validator that proposed the block.
    pub fn proposer_id(&self) -> ValidatorId {
        self.proposer_id
    }
}

impl State {
    /// Creates state with the given parameters.
    #[cfg_attr(feature = "cargo-clippy", allow(too_many_arguments))]
    pub fn new(
        validator_id: Option<ValidatorId>,
        consensus_public_key: PublicKey,
        consensus_secret_key: SecretKey,
        service_public_key: PublicKey,
        service_secret_key: SecretKey,
        tx_pool_capacity: usize,
        whitelist: Whitelist,
        stored: StoredConfiguration,
        connect: Connect,
        last_hash: Hash,
        last_height: Height,
        height_start_time: SystemTime,
    ) -> Self {
        State {
            validator_state: validator_id.map(ValidatorState::new),
            consensus_public_key,
            consensus_secret_key,
            service_public_key,
            service_secret_key,
            tx_pool_capacity: tx_pool_capacity,
            whitelist: whitelist,
            peers: HashMap::new(),
            connections: HashMap::new(),
            height: last_height,
            height_start_time,
            round: Round::zero(),
            locked_round: Round::zero(),
            locked_propose: None,
            last_hash,

            proposes: HashMap::new(),
            blocks: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),

            transactions: Arc::new(RwLock::new(BTreeMap::new())),

            queued: Vec::new(),

            unknown_txs: HashMap::new(),
            unknown_proposes_with_precommits: HashMap::new(),

            nodes_max_height: BTreeMap::new(),

            our_connect_message: connect,

            requests: HashMap::new(),

            timeout_adjuster: make_timeout_adjuster(&stored.consensus),
            propose_timeout: 0,
            config: stored,
        }
    }

    /// Returns `ValidatorState` if the node is validator.
    pub fn validator_state(&self) -> &Option<ValidatorState> {
        &self.validator_state
    }

    /// Returns validator id of the node if it is a validator. Returns `None` otherwise.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_state.as_ref().map(|s| s.id())
    }

    /// Updates the validator id. If there hasn't been `ValidatorState` for that id, then a new
    /// state will be created.
    pub fn renew_validator_id(&mut self, id: Option<ValidatorId>) {
        let validator_state = self.validator_state.take();
        self.validator_state = id.map(move |id| match validator_state {
            Some(mut state) => {
                state.set_validator_id(id);
                state
            }
            None => ValidatorState::new(id),
        });
    }

    /// Checks if the node is a validator.
    pub fn is_validator(&self) -> bool {
        self.validator_state().is_some()
    }

    /// Checks if the node is a leader for the current height and round.
    pub fn is_leader(&self) -> bool {
        self.validator_state()
            .as_ref()
            .map(|validator| self.leader(self.round()) == validator.id)
            .unwrap_or(false)
    }

    /// Returns node's whitelist.
    pub fn whitelist(&self) -> &Whitelist {
        &self.whitelist
    }

    /// Returns public (consensus and service) keys of known validators.
    pub fn validators(&self) -> &[ValidatorKeys] {
        &self.config.validator_keys
    }

    /// Returns `StoredConfiguration`.
    pub fn config(&self) -> &StoredConfiguration {
        &self.config
    }

    /// Returns validator id with a specified public key.
    pub fn find_validator(&self, peer: PublicKey) -> Option<ValidatorId> {
        self.validators()
            .iter()
            .position(|pk| pk.consensus_key == peer)
            .map(|id| ValidatorId(id as u16))
    }

    /// Returns `ConsensusConfig`.
    pub fn consensus_config(&self) -> &ConsensusConfig {
        &self.config.consensus
    }

    /// Returns `BTreeMap` with service configs identified by name.
    pub fn services_config(&self) -> &BTreeMap<String, Value> {
        &self.config.services
    }

    /// Replaces `StoredConfiguration` with a new one and updates validator id of the current node
    /// if the new config is different from the previous one.
    pub fn update_config(&mut self, config: StoredConfiguration) {
        if self.config == config {
            return;
        }

        trace!("Updating node config={:#?}", config);
        let validator_id = config
            .validator_keys
            .iter()
            .position(|pk| pk.consensus_key == *self.consensus_public_key())
            .map(|id| ValidatorId(id as u16));
        self.whitelist.set_validators(
            config.validator_keys.iter().map(|x| {
                x.consensus_key
            }),
        );
        self.renew_validator_id(validator_id);
        trace!("Validator={:#?}", self.validator_state());

        self.timeout_adjuster = make_timeout_adjuster(&config.consensus);
        self.config = config;
    }

    /// Adjusts propose timeout (see `TimeoutAdjuster` for the details).
    pub fn adjust_timeout(&mut self, snapshot: &Snapshot) {
        let timeout = self.timeout_adjuster.adjust_timeout(snapshot);
        self.propose_timeout = timeout;
    }

    /// Returns adjusted (see `TimeoutAdjuster` for the details) value of the propose timeout.
    pub fn propose_timeout(&self) -> Milliseconds {
        self.propose_timeout
    }

    /// Adds the public key, address, and `Connect` message of a validator.
    pub fn add_peer(&mut self, pubkey: PublicKey, msg: Connect) -> bool {
        self.connections.insert(msg.addr(), pubkey);
        self.peers.insert(pubkey, msg).is_none()
    }

    /// Removes a peer by the socket address.
    pub fn remove_peer_with_addr(&mut self, addr: &SocketAddr) -> bool {
        if let Some(pubkey) = self.connections.remove(addr) {
            self.peers.remove(&pubkey);
            return self.config.validator_keys.iter().any(|x| {
                x.consensus_key == pubkey
            });
        }
        false
    }

    /// Returns the keys of known peers with their `Connect` messages.
    pub fn peers(&self) -> &HashMap<PublicKey, Connect> {
        &self.peers
    }

    /// Returns public key of a validator identified by id.
    pub fn consensus_public_key_of(&self, id: ValidatorId) -> Option<PublicKey> {
        let id: usize = id.into();
        self.validators().get(id).map(|x| x.consensus_key)
    }

    /// Returns the consensus public key of the current node.
    pub fn consensus_public_key(&self) -> &PublicKey {
        &self.consensus_public_key
    }

    /// Returns the consensus secret key of the current node.
    pub fn consensus_secret_key(&self) -> &SecretKey {
        &self.consensus_secret_key
    }

    /// Returns the service public key of the current node.
    pub fn service_public_key(&self) -> &PublicKey {
        &self.service_public_key
    }

    /// Returns the service secret key of the current node.
    pub fn service_secret_key(&self) -> &SecretKey {
        &self.service_secret_key
    }

    /// Returns the leader id for the specified round and current height.
    pub fn leader(&self, round: Round) -> ValidatorId {
        let height: u64 = self.height().into();
        let round: u64 = round.into();
        ValidatorId(((height + round) % (self.validators().len() as u64)) as u16)
    }

    /// Returns the height for a validator identified by the public key.
    pub fn node_height(&self, key: &PublicKey) -> Height {
        *self.nodes_max_height.get(key).unwrap_or(&Height::zero())
    }

    /// Updates known height for a validator identified by the public key.
    pub fn set_node_height(&mut self, key: PublicKey, height: Height) {
        *self.nodes_max_height.entry(key).or_insert_with(
            Height::zero,
        ) = height;
    }

    /// Returns a list of nodes whose height is bigger than one of the current node.
    pub fn nodes_with_bigger_height(&self) -> Vec<&PublicKey> {
        self.nodes_max_height
            .iter()
            .filter(|&(_, h)| *h > self.height())
            .map(|(v, _)| v)
            .collect()
    }

    /// Returns sufficient number of votes for current validators number.
    pub fn majority_count(&self) -> usize {
        State::byzantine_majority_count(self.validators().len())
    }

    /// Returns sufficient number of votes for the given validators number.
    pub fn byzantine_majority_count(total: usize) -> usize {
        total * 2 / 3 + 1
    }

    /// Returns current height.
    pub fn height(&self) -> Height {
        self.height
    }

    /// Returns start time of the current height.
    pub fn height_start_time(&self) -> SystemTime {
        self.height_start_time
    }

    /// Returns the current round.
    pub fn round(&self) -> Round {
        self.round
    }

    /// Returns a hash of the last block.
    pub fn last_hash(&self) -> &Hash {
        &self.last_hash
    }

    /// Locks the node to the specified round and propose hash.
    ///
    /// # Panics
    ///
    /// Panics if the current "locked round" is bigger or equal to the new one.
    pub fn lock(&mut self, round: Round, hash: Hash) {
        if self.locked_round >= round {
            panic!("Incorrect lock")
        }
        self.locked_round = round;
        self.locked_propose = Some(hash);
    }

    /// Returns locked round number. Zero means that the node is not locked to any round.
    pub fn locked_round(&self) -> Round {
        self.locked_round
    }

    /// Returns propose hash on which the node makes lock.
    pub fn locked_propose(&self) -> Option<Hash> {
        self.locked_propose
    }

    /// Returns muttable propose state identified by hash.
    pub fn propose_mut(&mut self, hash: &Hash) -> Option<&mut ProposeState> {
        self.proposes.get_mut(hash)
    }
    /// Returns propose state identified by hash.
    pub fn propose(&self, hash: &Hash) -> Option<&ProposeState> {
        self.proposes.get(hash)
    }

    /// Returns a block with the specified hash.
    pub fn block(&self, hash: &Hash) -> Option<&BlockState> {
        self.blocks.get(hash)
    }

    /// Updates mode's round.
    pub fn jump_round(&mut self, round: Round) {
        self.round = round;
    }

    /// Increments node's round by one.
    pub fn new_round(&mut self) {
        self.round.increment();
    }

    /// Increments the node height by one and resets previous height data.
    // FIXME use block_hash
    pub fn new_height(&mut self, block_hash: &Hash, height_start_time: SystemTime) {
        self.height.increment();
        self.height_start_time = height_start_time;
        self.round = Round::first();
        self.locked_round = Round::zero();
        self.locked_propose = None;
        self.last_hash = *block_hash;
        {
            // Commit transactions if needed
            let txs = self.block(block_hash).unwrap().txs.clone();
            for hash in txs {
                self.transactions
                    .write()
                    .expect("Expected write lock")
                    .remove(&hash);
            }
        }
        // TODO: destruct/construct structure HeightState instead of call clear (ECR-171)
        self.blocks.clear();
        self.proposes.clear();
        self.prevotes.clear();
        self.precommits.clear();
        if let Some(ref mut validator_state) = self.validator_state {
            validator_state.clear();
        }
        self.requests.clear(); // FIXME: clear all timeouts (ECR-171)
    }

    /// Returns a list of queued consensus messages.
    pub fn queued(&mut self) -> Vec<ConsensusMessage> {
        let mut queued = Vec::new();
        ::std::mem::swap(&mut self.queued, &mut queued);
        queued
    }

    /// Add consensus message to the queue.
    pub fn add_queued(&mut self, msg: ConsensusMessage) {
        self.queued.push(msg);
    }

    /// Returns non-committed transactions.
    pub fn transactions(&self) -> &TxPool {
        &self.transactions
    }

    /// Adds a transaction to the pool and returns list of proposes that don't contain unknown
    /// transactions now.
    ///
    /// Transaction is ignored if the following criteria are fulfilled:
    /// - transactions pool size is exceeded
    /// - transaction isn't contained in unknown transaction list of any propose
    /// - transaction isn't a part of block
    pub fn add_transaction(
        &mut self,
        tx_hash: Hash,
        msg: Box<Transaction>,
        // if tx is in some of propose or in a block,
        // we should add it, or we could become stuck in some state
        mut high_priority_tx: bool,
    ) -> Vec<(Hash, Round)> {
        let mut full_proposes = Vec::new();
        for (propose_hash, propose_state) in &mut self.proposes {
            high_priority_tx |= propose_state.unknown_txs.remove(&tx_hash);
            if propose_state.unknown_txs.is_empty() {
                full_proposes.push((*propose_hash, propose_state.message().round()));
            }
        }
        let tx_pool_len = self.transactions.read().expect("Expected read lock").len();
        if tx_pool_len >= self.tx_pool_capacity {
            // but make warn about pool exceeded, even if we should add tx
            warn!(
                "Too many transactions in pool, txs={}, high_priority={}",
                tx_pool_len,
                high_priority_tx
            );
            if !high_priority_tx {
                return full_proposes;
            }
        }

        self.transactions
            .write()
            .expect("Expected read lock")
            .insert(tx_hash, msg);

        full_proposes
    }

    /// Returns pre-votes for the specified round and propose hash.
    pub fn prevotes(&self, round: Round, propose_hash: Hash) -> &[Prevote] {
        self.prevotes
            .get(&(round, propose_hash))
            .map(|votes| votes.messages().as_slice())
            .unwrap_or_else(|| &[])
    }

    /// Returns pre-commits for the specified round and propose hash.
    pub fn precommits(&self, round: Round, propose_hash: Hash) -> &[Precommit] {
        self.precommits
            .get(&(round, propose_hash))
            .map(|votes| votes.messages().as_slice())
            .unwrap_or_else(|| &[])
    }

    /// Returns `true` if this node has pre-vote for the specified round.
    ///
    /// # Panics
    ///
    /// Panics if this method is called for a non-validator node.
    pub fn have_prevote(&self, propose_round: Round) -> bool {
        if let Some(ref validator_state) = *self.validator_state() {
            validator_state.have_prevote(propose_round)
        } else {
            panic!("called have_prevote for auditor node")
        }
    }

    /// Adds propose from this node to the proposes list for the current height. Such propose
    /// cannot contain unknown transactions. Returns hash of the propose.
    pub fn add_self_propose(&mut self, msg: Propose) -> Hash {
        debug_assert!(self.validator_state().is_some());
        let propose_hash = msg.hash();
        self.proposes.insert(
            propose_hash,
            ProposeState {
                propose: msg,
                unknown_txs: HashSet::new(),
                block_hash: None,
            },
        );

        propose_hash
    }

    /// Adds propose from other node. Returns `ProposeState` if it is a new propose.
    pub fn add_propose(&mut self, msg: Propose) -> Option<&ProposeState> {
        let propose_hash = msg.hash();
        let txs = &self.transactions.read().expect("Expected read lock");
        match self.proposes.entry(propose_hash) {
            Entry::Occupied(..) => None,
            Entry::Vacant(e) => {
                let unknown_txs = msg.transactions()
                    .iter()
                    .filter(|tx| !txs.contains_key(tx))
                    .cloned()
                    .collect::<HashSet<Hash>>();
                for tx in &unknown_txs {
                    self.unknown_txs.entry(*tx).or_insert_with(Vec::new).push(
                        propose_hash,
                    );
                }
                Some(e.insert(ProposeState {
                    propose: msg.clone(),
                    unknown_txs: unknown_txs,
                    block_hash: None,
                }))
            }
        }
    }

    /// Adds block to the list of blocks for the current height. Returns `BlockState` if it is a
    /// new block.
    pub fn add_block(
        &mut self,
        block_hash: Hash,
        patch: Patch,
        txs: Vec<Hash>,
        proposer_id: ValidatorId,
    ) -> Option<&BlockState> {
        match self.blocks.entry(block_hash) {
            Entry::Occupied(..) => None,
            Entry::Vacant(e) => {
                Some(e.insert(BlockState {
                    hash: block_hash,
                    patch,
                    txs,
                    proposer_id,
                }))
            }
        }
    }

    /// Adds pre-vote. Returns `true` there are +2/3 pre-votes.
    ///
    /// # Panics
    ///
    /// A node panics if it has already sent a different `Prevote` for the same round.
    pub fn add_prevote(&mut self, msg: &Prevote) -> bool {
        let majority_count = self.majority_count();
        if let Some(ref mut validator_state) = self.validator_state {
            if validator_state.id == msg.validator() {
                if let Some(other) = validator_state.our_prevotes.insert(
                    msg.round(),
                    msg.clone(),
                )
                {
                    if &other != msg {
                        panic!(
                            "Trying to send different prevotes for the same round: \
                            old = {:?}, new = {:?}",
                            other,
                            msg
                        );
                    }
                }
            }
        }

        let key = (msg.round(), *msg.propose_hash());
        let validators_len = self.validators().len();
        let votes = self.prevotes.entry(key).or_insert_with(
            || Votes::new(validators_len),
        );
        votes.insert(msg);
        votes.count() >= majority_count
    }

    /// Returns `true` if there are +2/3 pre-votes for the specified round and hash.
    pub fn has_majority_prevotes(&self, round: Round, propose_hash: Hash) -> bool {
        match self.prevotes.get(&(round, propose_hash)) {
            Some(votes) => votes.count() >= self.majority_count(),
            None => false,
        }
    }

    /// Returns ids of validators that that sent pre-votes for the specified propose.
    pub fn known_prevotes(&self, round: Round, propose_hash: &Hash) -> BitVec {
        let len = self.validators().len();
        self.prevotes
            .get(&(round, *propose_hash))
            .map(|x| x.validators().clone())
            .unwrap_or_else(|| BitVec::from_elem(len, false))
    }

    /// Returns ids of validators that that sent pre-commits for the specified propose.
    pub fn known_precommits(&self, round: Round, propose_hash: &Hash) -> BitVec {
        let len = self.validators().len();
        self.precommits
            .get(&(round, *propose_hash))
            .map(|x| x.validators().clone())
            .unwrap_or_else(|| BitVec::from_elem(len, false))
    }

    /// Adds pre-commit. Returns `true` there are +2/3 pre-commits.
    ///
    /// # Panics
    ///
    /// A node panics if it has already sent a different `Precommit` for the same round.
    pub fn add_precommit(&mut self, msg: &Precommit) -> bool {
        let majority_count = self.majority_count();
        if let Some(ref mut validator_state) = self.validator_state {
            if validator_state.id == msg.validator() {
                if let Some(other) = validator_state.our_precommits.insert(
                    msg.round(),
                    msg.clone(),
                )
                {
                    if other.propose_hash() != msg.propose_hash() {
                        panic!(
                            "Trying to send different precommits for same round, old={:?}, \
                                new={:?}",
                            other,
                            msg
                        );
                    }
                }
            }
        }

        let key = (msg.round(), *msg.block_hash());
        let validators_len = self.validators().len();
        let votes = self.precommits.entry(key).or_insert_with(
            || Votes::new(validators_len),
        );
        votes.insert(msg);
        votes.count() >= majority_count
    }

    /// Adds unknown (for this node) propose.
    pub fn add_unknown_propose_with_precommits(
        &mut self,
        round: Round,
        propose_hash: Hash,
        block_hash: Hash,
    ) {
        self.unknown_proposes_with_precommits
            .entry(propose_hash)
            .or_insert_with(Vec::new)
            .push((round, block_hash));
    }

    /// Removes propose from the list of unknown proposes and returns its round and hash.
    pub fn unknown_propose_with_precommits(&mut self, propose_hash: &Hash) -> Vec<(Round, Hash)> {
        self.unknown_proposes_with_precommits
            .remove(propose_hash)
            .unwrap_or_default()
    }

    /// Returns true if the node has +2/3 pre-commits for the specified round and block hash.
    pub fn has_majority_precommits(&self, round: Round, block_hash: Hash) -> bool {
        match self.precommits.get(&(round, block_hash)) {
            Some(votes) => votes.count() >= self.majority_count(),
            None => false,
        }
    }

    /// Returns `true` if the node doesn't have proposes different from the locked one.
    pub fn have_incompatible_prevotes(&self) -> bool {
        for round in self.locked_round.next().iter_to(self.round.next()) {
            match self.validator_state {
                Some(ref validator_state) => {
                    if let Some(msg) = validator_state.our_prevotes.get(&round) {
                        // TODO: unefficient (ECR-171)
                        if Some(*msg.propose_hash()) != self.locked_propose {
                            return true;
                        }
                    }
                }
                None => unreachable!(),
            }
        }
        false
    }

    /// Adds data-request to the queue. Returns `true` if it is a new request.
    pub fn request(&mut self, data: RequestData, peer: PublicKey) -> bool {
        let state = self.requests.entry(data).or_insert_with(RequestState::new);
        let is_new = state.is_empty();
        state.insert(peer);
        is_new
    }

    /// Returns public key of a peer that has required information. Returned key is removed from
    /// the corresponding validators list, so next time request will be sent to a different peer.
    pub fn retry(&mut self, data: &RequestData, peer: Option<PublicKey>) -> Option<PublicKey> {
        let next = {
            let state = if let Some(state) = self.requests.get_mut(data) {
                state
            } else {
                return None;
            };
            if let Some(peer) = peer {
                state.remove(&peer);
            }
            state.peek()
        };

        if next.is_none() {
            self.requests.remove(data);
        };
        next
    }

    /// Removes the specified request from the pending request list.
    pub fn remove_request(&mut self, data: &RequestData) -> HashSet<PublicKey> {
        let state = self.requests.remove(data);
        state.map(|s| s.known_nodes).unwrap_or_default()
    }

    /// Returns the `Connect` message of the current node.
    pub fn our_connect_message(&self) -> &Connect {
        &self.our_connect_message
    }

    /// Updates the `Connect` message of the current node.
    pub fn set_our_connect_message(&mut self, msg: Connect) {
        self.our_connect_message = msg;
    }
}

fn make_timeout_adjuster(config: &ConsensusConfig) -> Box<TimeoutAdjuster> {
    match config.timeout_adjuster {
        TimeoutAdjusterConfig::Constant { timeout } => Box::new(Constant::new(timeout)),
        TimeoutAdjusterConfig::Dynamic {
            min,
            max,
            threshold,
        } => Box::new(Dynamic::new(min, max, threshold)),
        TimeoutAdjusterConfig::MovingAverage {
            min,
            max,
            adjustment_speed,
            optimal_block_load,
        } => Box::new(MovingAverage::new(
            min,
            max,
            adjustment_speed,
            config.txs_block_limit,
            optimal_block_load,
        )),
    }
}
