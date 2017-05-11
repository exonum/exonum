use serde_json::Value;

use std::collections::{BTreeMap, HashMap, HashSet, BTreeSet};
use std::collections::hash_map::Entry;
use std::net::SocketAddr;
use std::time::{SystemTime, Duration};

use messages::{Message, Propose, Prevote, Precommit, ConsensusMessage, Connect, BitVec};
use crypto::{PublicKey, SecretKey, Hash};
use storage::Patch;
use events::Milliseconds;
use blockchain::{ConsensusConfig, StoredConfiguration, Transaction};

// TODO: replace by in disk tx pool
const TX_POOL_LIMIT: usize = 20000;

// TODO: move request timeouts into node configuration

pub const REQUEST_PROPOSE_TIMEOUT: Milliseconds = 100;
pub const REQUEST_TRANSACTIONS_TIMEOUT: Milliseconds = 100;
pub const REQUEST_PREVOTES_TIMEOUT: Milliseconds = 100;
pub const REQUEST_BLOCK_TIMEOUT: Milliseconds = 100;

pub type Round = u32;
pub type Height = u64;
pub type ValidatorId = u32;
// TODO replace by persistent TxPool
pub type TxPool = BTreeMap<Hash, Box<Transaction>>;
// TODO: reduce copying of Hash

pub struct State {
    validator_state: Option<ValidatorState>,
    our_connect_message: Connect,

    public_key: PublicKey,
    secret_key: SecretKey,
    config: StoredConfiguration,

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

    // maximum of node heigt in consensus messages
    nodes_max_height: BTreeMap<PublicKey, Height>,
}

#[derive(Debug, Clone)]
pub struct ValidatorState {
    id: ValidatorId,
    our_prevotes: HashMap<Round, Prevote>,
    our_precommits: HashMap<Round, Precommit>,
}

// Required data specific for some height.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestData {
    Propose(Hash),
    Transactions(Hash),
    Prevotes(Round, Hash),
    Block(Height),
}

struct RequestState {
    // Number of attempts made.
    retries: u16,
    // Nodes that have the required information.
    known_nodes: HashSet<PublicKey>,
}

pub struct ProposeState {
    hash: Hash,
    propose: Propose,
    // FIXME: use HashSet here
    unknown_txs: BTreeSet<Hash>,
}

#[derive(Clone)]
pub struct BlockState {
    hash: Hash,
    // Changes that should be made for block committing.
    patch: Patch,
    txs: Vec<Hash>,
    propose_round: Round,
}

pub trait VoteMessage: Message + Clone {
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

pub struct Votes<T: VoteMessage> {
    messages: Vec<T>,
    validators: BitVec,
    count: usize,
}

impl ValidatorState {
    pub fn new(id: ValidatorId) -> ValidatorState {
        ValidatorState {
            id: id,
            our_precommits: HashMap::new(),
            our_prevotes: HashMap::new(),
        }
    }

    pub fn id(&self) -> ValidatorId {
        self.id
    }

    pub fn set_validator_id(&mut self, id: ValidatorId) {
        self.id = id;
    }

    pub fn have_prevote(&self, round: Round) -> bool {
        self.our_prevotes.get(&round).is_some()
    }

    pub fn clear(&mut self) {
        self.our_precommits.clear();
        self.our_prevotes.clear();
    }
}

impl<T> Votes<T>
    where T: VoteMessage
{
    pub fn new(validators_len: usize) -> Votes<T> {
        Votes {
            messages: Vec::new(),
            validators: BitVec::from_elem(validators_len, false),
            count: 0,
        }
    }

    pub fn insert(&mut self, message: &T) {
        let voter = message.validator() as usize;
        if !self.validators[voter] {
            self.count += 1;
            self.validators.set(voter, true);
            self.messages.push(message.clone());
        }
    }

    pub fn validators(&self) -> &BitVec {
        &self.validators
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn messages(&self) -> &Vec<T> {
        &self.messages
    }
}

impl RequestData {
    pub fn timeout(&self) -> Duration {
        #![cfg_attr(feature="clippy", allow(match_same_arms))]
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
    pub fn hash(&self) -> Hash {
        self.hash
    }

    pub fn message(&self) -> &Propose {
        &self.propose
    }

    pub fn unknown_txs(&self) -> &BTreeSet<Hash> {
        &self.unknown_txs
    }

    pub fn has_unknown_txs(&self) -> bool {
        !self.unknown_txs.is_empty()
    }
}

impl BlockState {
    pub fn new(hash: Hash, patch: Patch, txs: Vec<Hash>, propose_round: Round) -> BlockState {
        BlockState {
            hash: hash,
            patch: patch,
            txs: txs,
            propose_round: propose_round,
        }
    }

    pub fn hash(&self) -> Hash {
        self.hash
    }

    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    pub fn txs(&self) -> &Vec<Hash> {
        &self.txs
    }

    pub fn propose_round(&self) -> Round {
        self.propose_round
    }
}

impl State {
    #![cfg_attr(feature="clippy", allow(too_many_arguments))]
    pub fn new(validator_id: Option<ValidatorId>,
               public_key: PublicKey,
               secret_key: SecretKey,
               stored: StoredConfiguration,
               connect: Connect,
               last_hash: Hash,
               last_height: u64,
               height_start_time: SystemTime)
               -> State {

        State {
            validator_state: validator_id.map(ValidatorState::new),
            public_key: public_key,
            secret_key: secret_key,
            peers: HashMap::new(),
            connections: HashMap::new(),
            height: last_height,
            height_start_time: height_start_time,
            round: 0,
            locked_round: 0,
            locked_propose: None,
            last_hash: last_hash,

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

    pub fn validator_state(&self) -> &Option<ValidatorState> {
        &self.validator_state
    }

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

    pub fn is_validator(&self) -> bool {
        self.validator_state().is_some()
    }

    pub fn is_leader(&self) -> bool {
        self.validator_state()
            .as_ref()
            .map(|validator| self.leader(self.round()) == validator.id)
            .unwrap_or(false)
    }

    pub fn validators(&self) -> &[PublicKey] {
        &self.config.validators
    }

    pub fn config(&self) -> &StoredConfiguration {
        &self.config
    }

    pub fn find_validator(&self, peer: &PublicKey) -> Option<ValidatorId> {
        self.validators()
            .iter()
            .position(|pk| pk == peer)
            .map(|id| id as ValidatorId)
    }

    pub fn consensus_config(&self) -> &ConsensusConfig {
        &self.config.consensus
    }

    pub fn services_config(&self) -> &BTreeMap<String, Value> {
        &self.config.services
    }

    pub fn update_config(&mut self, config: StoredConfiguration) {
        trace!("Updating node config={:#?}", config);
        let validator_id = config
            .validators
            .iter()
            .position(|pk| pk == self.public_key())
            .map(|id| id as u32);
        self.renew_validator_id(validator_id);
        trace!("Validator={:#?}", self.validator_state());
        self.config = config;
    }

    pub fn propose_timeout(&self) -> Milliseconds {
        self.config.consensus.propose_timeout
    }

    pub fn set_propose_timeout(&mut self, timeout: Milliseconds) {
        debug_assert!(timeout < self.config.consensus.round_timeout);
        self.config.consensus.propose_timeout = timeout;
    }

    pub fn add_peer(&mut self, pubkey: PublicKey, msg: Connect) -> bool {
        self.connections.insert(msg.addr(), pubkey);
        self.peers.insert(pubkey, msg).is_none()
    }

    pub fn remove_peer_with_addr(&mut self, addr: &SocketAddr) -> bool {
        if let Some(pubkey) = self.connections.remove(addr) {
            self.peers.remove(&pubkey);
            return self.config.validators.contains(&pubkey);
        }
        false
    }

    pub fn peers(&self) -> &HashMap<PublicKey, Connect> {
        &self.peers
    }

    pub fn public_key_of(&self, id: ValidatorId) -> Option<&PublicKey> {
        self.validators().get(id as usize)
    }

    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    pub fn leader(&self, round: Round) -> ValidatorId {
        ((self.height() + round as u64) % (self.validators().len() as u64)) as ValidatorId
    }

    pub fn node_height(&self, key: &PublicKey) -> Height {
        *self.nodes_max_height.get(key).unwrap_or(&0)
    }

    pub fn set_node_height(&mut self, key: PublicKey, height: Height) {
        *self.nodes_max_height.entry(key).or_insert(0) = height;
    }

    pub fn nodes_with_bigger_height(&self) -> Vec<&PublicKey> {
        self.nodes_max_height
            .iter()
            .filter(|&(_, h)| *h > self.height())
            .map(|(v, _)| v)
            .collect()
    }

    pub fn majority_count(&self) -> usize {
        // FIXME: What if validators count < 4?
        //self.validators().len() * 2 / 3 + 1
        State::byzantine_majority_count(self.validators().len())
    }

    pub fn byzantine_majority_count(total: usize) -> usize {
        total * 2 / 3 + 1
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    pub fn height_start_time(&self) -> SystemTime {
        self.height_start_time
    }

    pub fn round(&self) -> Round {
        self.round
    }

    pub fn last_hash(&self) -> &Hash {
        &self.last_hash
    }

    pub fn lock(&mut self, round: Round, hash: Hash) {
        if self.locked_round >= round {
            panic!("Incorrect lock")
        }
        self.locked_round = round;
        self.locked_propose = Some(hash);
    }

    pub fn locked_round(&self) -> Round {
        self.locked_round
    }

    pub fn locked_propose(&self) -> Option<Hash> {
        self.locked_propose
    }

    pub fn propose(&self, hash: &Hash) -> Option<&ProposeState> {
        self.proposes.get(hash)
    }

    pub fn block(&self, hash: &Hash) -> Option<&BlockState> {
        self.blocks.get(hash)
    }

    pub fn jump_round(&mut self, round: Round) {
        self.round = round;
    }

    pub fn new_round(&mut self) {
        self.round += 1;
    }

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

    pub fn queued(&mut self) -> Vec<ConsensusMessage> {
        let mut queued = Vec::new();
        ::std::mem::swap(&mut self.queued, &mut queued);
        queued
    }

    pub fn add_queued(&mut self, msg: ConsensusMessage) {
        self.queued.push(msg);
    }

    pub fn transactions(&self) -> &TxPool {
        &self.transactions
    }

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

    pub fn prevotes(&self, round: Round, propose_hash: Hash) -> &[Prevote] {
        self.prevotes
            .get(&(round, propose_hash))
            .map(|votes| votes.messages().as_slice())
            .unwrap_or_else(|| &[])
    }

    pub fn precommits(&self, round: Round, propose_hash: Hash) -> &[Precommit] {
        self.precommits
            .get(&(round, propose_hash))
            .map(|votes| votes.messages().as_slice())
            .unwrap_or_else(|| &[])
    }

    pub fn have_prevote(&self, propose_round: Round) -> bool {
        if let Some(ref validator_state) = *self.validator_state() {
            validator_state.have_prevote(propose_round)
        } else {
            panic!("called have_prevote for auditor node")
        }
    }

    pub fn add_self_propose(&mut self, msg: Propose) -> Hash {
        debug_assert!(self.validator_state().is_some());
        let propose_hash = msg.hash();
        self.proposes
            .insert(propose_hash,
                    ProposeState {
                        hash: propose_hash,
                        propose: msg,
                        unknown_txs: BTreeSet::new(),
                    });

        propose_hash
    }

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
                    .collect::<BTreeSet<Hash>>();
                for tx in &unknown_txs {
                    self.unknown_txs
                        .entry(*tx)
                        .or_insert_with(Vec::new)
                        .push(propose_hash);
                }
                Some(e.insert(ProposeState {
                                  hash: propose_hash,
                                  propose: msg.clone(),
                                  unknown_txs: unknown_txs,
                              }))
            }
        }
    }

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

    pub fn add_prevote(&mut self, msg: &Prevote) -> bool {
        let majority_count = self.majority_count();
        if let Some(ref mut validator_state) = self.validator_state {
            if validator_state.id == msg.validator() {
                if let Some(other) = validator_state
                       .our_prevotes
                       .insert(msg.round(), msg.clone()) {
                    if &other != msg {
                        panic!("Trying to send different prevotes for same round, old={:?}, \
                                new={:?}",
                                other,
                                msg);
                    }
                }
            }
        }

        let key = (msg.round(), *msg.propose_hash());
        let validators_len = self.validators().len();
        let mut votes = self.prevotes
            .entry(key)
            .or_insert_with(|| Votes::new(validators_len));
        votes.insert(msg);
        votes.count() >= majority_count
    }

    pub fn has_majority_prevotes(&self, round: Round, propose_hash: Hash) -> bool {
        match self.prevotes.get(&(round, propose_hash)) {
            Some(votes) => votes.count() >= self.majority_count(),
            None => false,
        }
    }

    pub fn known_prevotes(&self, round: Round, propose_hash: &Hash) -> BitVec {
        let len = self.validators().len();
        self.prevotes
            .get(&(round, *propose_hash))
            .map(|x| x.validators().clone())
            .unwrap_or_else(|| BitVec::from_elem(len, false))
    }

    pub fn known_precommits(&self, round: Round, propose_hash: &Hash) -> BitVec {
        let len = self.validators().len();
        self.precommits
            .get(&(round, *propose_hash))
            .map(|x| x.validators().clone())
            .unwrap_or_else(|| BitVec::from_elem(len, false))
    }

    pub fn add_precommit(&mut self, msg: &Precommit) -> bool {
        let majority_count = self.majority_count();
        if let Some(ref mut validator_state) = self.validator_state {
            if validator_state.id == msg.validator() {
                if let Some(other) = validator_state
                       .our_precommits
                       .insert(msg.round(), msg.clone()) {
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
        let votes = self.precommits
            .entry(key)
            .or_insert_with(|| Votes::new(validators_len));
        votes.insert(msg);
        votes.count() >= majority_count
    }

    pub fn add_unknown_propose_with_precommits(&mut self,
                                               round: Round,
                                               propose_hash: Hash,
                                               block_hash: Hash) {
        self.unknown_proposes_with_precommits
            .entry(propose_hash)
            .or_insert_with(Vec::new)
            .push((round, block_hash));
    }

    pub fn unknown_propose_with_precommits(&mut self, propose_hash: &Hash) -> Vec<(Round, Hash)> {
        self.unknown_proposes_with_precommits
            .remove(propose_hash)
            .unwrap_or_default()
    }

    pub fn has_majority_precommits(&self, round: Round, block_hash: Hash) -> bool {
        match self.precommits.get(&(round, block_hash)) {
            Some(votes) => votes.count() >= self.majority_count(),
            None => false,
        }
    }

    pub fn have_incompatible_prevotes(&self) -> bool {
        for round in self.locked_round + 1...self.round {
            match self.validator_state {
                Some(ref validator_state) => {
                    if let Some(msg) = validator_state.our_prevotes.get(&round) {
                        // TODO: unefficient
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

    pub fn request(&mut self, data: RequestData, peer: PublicKey) -> bool {
        let mut state = self.requests.entry(data).or_insert_with(RequestState::new);
        let is_new = state.is_empty();
        state.insert(peer);
        is_new
    }

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

    pub fn remove_request(&mut self, data: &RequestData) -> HashSet<PublicKey> {
        let state = self.requests.remove(data);
        state.map(|s| s.known_nodes).unwrap_or_default()
    }

    pub fn our_connect_message(&self) -> &Connect {
        &self.our_connect_message
    }

    pub fn set_our_connect_message(&mut self, msg: Connect) {
        self.our_connect_message = msg;
    }
}
