//! State of the `NodeHandler`.

use serde_json::Value;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::net::SocketAddr;
use std::time::{SystemTime, Duration};

use messages::{Message, Propose, Prevote, Precommit, ConsensusMessage, Connect, BitVec};
use crypto::{PublicKey, SecretKey, Hash};
use storage::Patch;
use events::Milliseconds;
use blockchain::{ConsensusConfig, StoredConfiguration, Transaction};
use node::whitelist::Whitelist;

// TODO: replace by in disk tx pool
const TX_POOL_LIMIT: usize = 20000;

// TODO: move request timeouts into node configuration

/// Timeout value for the `RequestPropose` message.
pub const REQUEST_PROPOSE_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `RequestTransactions` message.
pub const REQUEST_TRANSACTIONS_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `RequestPrevotes` message.
pub const REQUEST_PREVOTES_TIMEOUT: Milliseconds = 100;
/// Timeout value for the `RequestBlock` message.
pub const REQUEST_BLOCK_TIMEOUT: Milliseconds = 100;

/// Consensus round index.
pub type Round = u32;
/// Blockchain's height (number of blocks).
pub type Height = u64;
/// Validators id.
pub type ValidatorId = u32;

/// Transactions pool.
// TODO replace by persistent TxPool
pub type TxPool = BTreeMap<Hash, Box<Transaction>>;
// TODO: reduce copying of Hash

/// State of the `NodeHandler`.
#[derive(Debug)]
pub struct State {
    validator_state: Option<ValidatorState>,
    our_connect_message: Connect,

    public_key: PublicKey,
    secret_key: SecretKey,
    config: StoredConfiguration,
    whitelist: Whitelist,

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

    // TODO: add hashmap of transactions we wait for
    transactions: TxPool,

    queued: Vec<ConsensusMessage>,

    unknown_txs: HashMap<Hash, Vec<Hash>>,
    unknown_proposes_with_precommits: HashMap<Hash, Vec<(Round, Hash)>>,

    // Our requests state.
    requests: HashMap<RequestData, RequestState>,

    // maximum of node height in consensus messages
    nodes_max_height: BTreeMap<PublicKey, Height>,
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
    /// Represents `RequestPropose` message.
    Propose(Hash),
    /// Represents `RequestTransactions` message.
    Transactions(Hash),
    /// Represents `RequestPrevotes` message.
    Prevotes(Round, Hash),
    /// Represents `RequestBlock` message.
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
}

/// State of a block.
#[derive(Clone, Debug)]
pub struct BlockState {
    hash: Hash,
    // Changes that should be made for block committing.
    patch: Patch,
    txs: Vec<Hash>,
    propose_round: Round,
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
            our_prevotes: HashMap::new()
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
    where T: VoteMessage
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
        let voter = message.validator() as usize;
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
            RequestData::Propose(..) => REQUEST_PROPOSE_TIMEOUT,
            RequestData::Transactions(..) => REQUEST_TRANSACTIONS_TIMEOUT,
            RequestData::Prevotes(..) => REQUEST_PREVOTES_TIMEOUT,
            RequestData::Block(..) => REQUEST_BLOCK_TIMEOUT,
        };
        Duration::from_millis(ms)
    }
}

impl RequestState {
    fn new() -> RequestState {
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
    pub fn new(hash: Hash, patch: Patch, txs: Vec<Hash>, propose_round: Round) -> BlockState {
        BlockState {
            hash: hash,
            patch: patch,
            txs: txs,
            propose_round: propose_round,
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

    /// Returns round number in which block was proposed.
    pub fn propose_round(&self) -> Round {
        self.propose_round
    }
}

impl State {
    /// Creates state with the given parameters.
    #[cfg_attr(feature="cargo-clippy", allow(too_many_arguments))]
    pub fn new(validator_id: Option<ValidatorId>,
               public_key: PublicKey,
               secret_key: SecretKey,
               whitelist: Whitelist,
               stored: StoredConfiguration,
               connect: Connect,
               last_hash: Hash,
               last_height: u64,
               height_start_time: SystemTime)
               -> Self {
        State {
            validator_state: validator_id.map(ValidatorState::new),
            public_key,
            secret_key,
            whitelist: whitelist,
            peers: HashMap::new(),
            connections: HashMap::new(),
            height: last_height,
            height_start_time,
            round: 0,
            locked_round: 0,
            locked_propose: None,
            last_hash,

            proposes: HashMap::new(),
            blocks: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),

            transactions: TxPool::new(),

            queued: Vec::new(),

            unknown_txs: HashMap::new(),
            unknown_proposes_with_precommits: HashMap::new(),

            nodes_max_height: BTreeMap::new(),

            our_connect_message: connect,

            requests: HashMap::new(),

            config: stored,
        }
    }

    /// Returns `ValidatorState` if the node is validator.
    pub fn validator_state(&self) -> &Option<ValidatorState> {
        &self.validator_state
    }

    /// Updates the validator id. If there hasn't been `ValidatorState` for that id, then a new
    /// state will be created.
    pub fn renew_validator_id(&mut self, id: Option<ValidatorId>) {
        let validator_state = self.validator_state.take();
        self.validator_state = id.map(move |id|{
            match validator_state {
                Some(mut state) => {
                    state.set_validator_id(id);
                    state
                },
                None => ValidatorState::new(id),
            }
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

    pub fn whitelist(&self) -> &Whitelist {
        &self.whitelist
    }

    /// Returns public keys of known validators.
    pub fn validators(&self) -> &[PublicKey] {
        &self.config.validators
    }

    /// Returns `StoredConfiguration`.
    pub fn config(&self) -> &StoredConfiguration {
        &self.config
    }

    /// Returns validator id with a specified public key.
    pub fn find_validator(&self, peer: &PublicKey) -> Option<ValidatorId> {
        self.validators()
            .iter()
            .position(|pk| pk == peer)
            .map(|id| id as ValidatorId)
    }

    /// Returns `ConsensusConfig`.
    pub fn consensus_config(&self) -> &ConsensusConfig {
        &self.config.consensus
    }

    /// Returns `BTreeMap` with service configs identified by name.
    pub fn services_config(&self) -> &BTreeMap<String, Value> {
        &self.config.services
    }

    /// Replaces `StoredConfiguration` with a new one and updates validator id of the current node.
    pub fn update_config(&mut self, config: StoredConfiguration) {
        trace!("Updating node config={:#?}", config);
        let validator_id = config.validators
                            .iter()
                            .position(|pk| pk == self.public_key())
                            .map(|id| id as u32);
        self.whitelist.set_validators(config.validators.iter().cloned());
        self.renew_validator_id(validator_id);
        trace!("Validator={:#?}", self.validator_state());
        self.config = config;
    }

    /// Returns value of the propose timeout from `ConsensusConfig`.
    pub fn propose_timeout(&self) -> Milliseconds {
        self.config.consensus.propose_timeout
    }

    /// Updates propose timeout value.
    pub fn set_propose_timeout(&mut self, timeout: Milliseconds) {
        debug_assert!(timeout < self.config.consensus.round_timeout);
        self.config.consensus.propose_timeout = timeout;
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
            return self.config.validators.contains(&pubkey);
        }
        false
    }

    /// Returns the keys of known peers with their `Connect` messages.
    pub fn peers(&self) -> &HashMap<PublicKey, Connect> {
        &self.peers
    }

    /// Returns public key of a validator identified by id.
    pub fn public_key_of(&self, id: ValidatorId) -> Option<&PublicKey> {
        self.validators().get(id as usize)
    }

    /// Returns the public key of the current node.
    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    /// Returns the secret key of the current node.
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    /// Returns the leader id for the specified round and current height.
    pub fn leader(&self, round: Round) -> ValidatorId {
        ((self.height() + round as u64) % (self.validators().len() as u64)) as ValidatorId
    }

    /// Returns the height for a validator identified by the public key.
    pub fn node_height(&self, key: &PublicKey) -> Height {
        *self.nodes_max_height.get(key).unwrap_or(&0)
    }

    /// Updates known height for a validator identified by the public key.
    pub fn set_node_height(&mut self, key: PublicKey, height: Height) {
        *self.nodes_max_height.entry(key).or_insert(0) = height;
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
    pub fn height(&self) -> u64 {
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
        self.round += 1;
    }

    /// Increments the node height by one and resets previous height data.
    // FIXME use block_hash
    pub fn new_height(&mut self, block_hash: &Hash, height_start_time: SystemTime) {
        self.height += 1;
        self.height_start_time = height_start_time;
        self.round = 1;
        self.locked_round = 0;
        self.locked_propose = None;
        self.last_hash = *block_hash;
        {
            // Commit transactions if needed
            let txs = self.block(block_hash).unwrap().txs.clone();
            for hash in txs {
                self.transactions.remove(&hash);
            }
        }
        // TODO: destruct/construct structure HeightState instead of call clear
        self.blocks.clear();
        self.proposes.clear();
        self.prevotes.clear();
        self.precommits.clear();
        if let Some(ref mut validator_state) = self.validator_state {
            validator_state.clear();
        }
        self.requests.clear(); // FIXME: clear all timeouts
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
    pub fn add_transaction(&mut self, tx_hash: Hash, msg: Box<Transaction>) -> Vec<(Hash, Round)> {
        let mut full_proposes = Vec::new();
        // if tx is in some of propose, we should add it, or we can stuck on some state
        let mut high_priority_tx = false;
        for (propose_hash, propose_state) in &mut self.proposes {
            high_priority_tx |= propose_state.unknown_txs.remove(&tx_hash);
            if propose_state.unknown_txs.is_empty() {
                full_proposes.push((*propose_hash, propose_state.message().round()));
            }
        }

        if self.transactions.len() >= TX_POOL_LIMIT {
            // but make warn about pool exceeded, even if we should add tx
            warn!("Too many transactions in pool, txs={}, high_priority={}",
                  self.transactions.len(),
                  high_priority_tx);
            if !high_priority_tx {
                return full_proposes;
            }
        }

        self.transactions.insert(tx_hash, msg);

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
        if let Some(ref validator_state) = *self.validator_state(){
            validator_state.have_prevote(propose_round)
        }
        else {
            panic!("called have_prevote for auditor node")
        }
    }

    /// Adds propose from this node to the proposes list for the current height. Such propose
    /// cannot contain unknown transactions. Returns hash of the propose.
    pub fn add_self_propose(&mut self, msg: Propose) -> Hash {
        debug_assert!(self.validator_state().is_some());
        let propose_hash = msg.hash();
        self.proposes.insert(propose_hash,
                             ProposeState {
                                 propose: msg,
                                 unknown_txs: HashSet::new(),
                             });

        propose_hash
    }

    /// Adds propose from other node. Returns `ProposeState` if it is a new propose.
    pub fn add_propose(&mut self, msg: Propose) -> Option<&ProposeState> {
        let propose_hash = msg.hash();
        let txs = &self.transactions;
        match self.proposes.entry(propose_hash) {
            Entry::Occupied(..) => None,
            Entry::Vacant(e) => {
                let unknown_txs = msg.transactions()
                    .iter()
                    .filter(|tx| !txs.contains_key(tx))
                    .cloned()
                    .collect::<HashSet<Hash>>();
                for tx in &unknown_txs {
                    self.unknown_txs
                        .entry(*tx)
                        .or_insert_with(Vec::new)
                        .push(propose_hash);
                }
                Some(e.insert(ProposeState {
                    propose: msg.clone(),
                    unknown_txs: unknown_txs,
                }))
            }
        }
    }

    /// Adds block to the list of blocks for the current height. Returns `BlockState` if it is a
    /// new block.
    pub fn add_block(&mut self,
                     block_hash: Hash,
                     patch: Patch,
                     txs: Vec<Hash>,
                     propose_round: Round)
                     -> Option<&BlockState> {
        match self.blocks.entry(block_hash) {
            Entry::Occupied(..) => None,
            Entry::Vacant(e) => {
                Some(e.insert(BlockState {
                    hash: block_hash,
                    patch: patch,
                    txs: txs,
                    propose_round: propose_round,
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
                if let Some(other) = validator_state.our_prevotes.insert(msg.round(), msg.clone()) {
                    if &other != msg {
                        panic!("Trying to send different prevotes for the same round, old={:?}, new={:?}",
                                other,
                                msg);
                    }
                }
            }
        }

        let key = (msg.round(), *msg.propose_hash());
        let validators_len = self.validators().len();
        let mut votes = self.prevotes.entry(key).or_insert_with(|| Votes::new(validators_len));
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
                if let Some(other) = validator_state.our_precommits.insert(msg.round(), msg.clone()) {
                    if other.propose_hash() != msg.propose_hash() {
                        panic!("Trying to send different precommits for same round, old={:?}, \
                                new={:?}",
                                other,
                                msg);
                    }
                }
            }
        }

        let key = (msg.round(), *msg.block_hash());
        let validators_len = self.validators().len();
        let votes = self.precommits.entry(key).or_insert_with(|| Votes::new(validators_len));
        votes.insert(msg);
        votes.count() >= majority_count
    }

    /// Adds unknown (for this node) propose.
    pub fn add_unknown_propose_with_precommits(&mut self,
                                               round: Round,
                                               propose_hash: Hash,
                                               block_hash: Hash) {
        self.unknown_proposes_with_precommits
            .entry(propose_hash)
            .or_insert_with(Vec::new)
            .push((round, block_hash));
    }

    /// Removes propose from the list of unknown proposes and returns its round and hash.
    pub fn unknown_propose_with_precommits(&mut self, propose_hash: &Hash) -> Vec<(Round, Hash)> {
        self.unknown_proposes_with_precommits.remove(propose_hash).unwrap_or_default()
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
        for round in self.locked_round + 1..self.round + 1 {
            match self.validator_state {
                Some(ref validator_state) => {
                    if let Some(msg) = validator_state.our_prevotes.get(&round) {
                        // TODO: unefficient
                        if Some(*msg.propose_hash()) != self.locked_propose {
                            return true;
                        }
                    }
                }
                None => unreachable!()
            }
        }
        false
    }

    /// Adds data-request to the queue. Returns `true` if it is a new request.
    pub fn request(&mut self, data: RequestData, peer: PublicKey) -> bool {
        let mut state = self.requests
            .entry(data)
            .or_insert_with(RequestState::new);
        let is_new = state.is_empty();
        state.insert(peer);
        is_new
    }

    /// Returns public key of a peer that has required information. Returned key is removed from
    /// the corresponding validators list, so next time request will be sent to a different peer.
    pub fn retry(&mut self, data: &RequestData, peer: Option<PublicKey>) -> Option<PublicKey> {
        let next = {
            let mut state = if let Some(state) = self.requests.get_mut(data) {
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
