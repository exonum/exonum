use std::collections::{HashMap, HashSet, BTreeSet};
use std::collections::hash_map::Entry;
use std::net::SocketAddr;

use time::Duration;

use super::super::messages::{Message, Propose, Prevote, Precommit, ConsensusMessage, Connect,
                             BitVec};
use super::super::crypto::{PublicKey, Hash};
use super::super::storage::Patch;

// TODO: move request timeouts into node configuration

const REQUEST_PROPOSE_WAIT: u64 = 100; // milliseconds
const REQUEST_TRANSACTIONS_WAIT: u64 = 100;
const REQUEST_PREVOTES_WAIT: u64 = 100;
const REQUEST_PRECOMMITS_WAIT: u64 = 100;
const REQUEST_BLOCK_WAIT: u64 = 100;

pub type Round = u32;
pub type Height = u64;
pub type ValidatorId = u32;

// TODO: reduce copying of Hash

pub struct State<Tx> {
    id: u32,
    peers: HashMap<PublicKey, Connect>,
    validators: Vec<PublicKey>,
    connections: HashMap<SocketAddr, PublicKey>,
    height: u64,
    round: Round,
    locked_round: Round,
    locked_propose: Option<Hash>,
    last_hash: Hash,

    // messages
    proposes: HashMap<Hash, ProposeState>,
    blocks: HashMap<Hash, BlockState>,
    prevotes: HashMap<(Round, Hash), HashMap<ValidatorId, Prevote>>,
    precommits: HashMap<(Round, Hash), HashMap<ValidatorId, Precommit>>,

    transactions: HashMap<Hash, Tx>,
    queued: Vec<ConsensusMessage>,

    unknown_txs: HashMap<Hash, Vec<Hash>>,
    unknown_proposes_with_precommits: HashMap<Hash, Vec<(Round, Hash)>>,

    // TODO: add hashmap of transactions we wait for
    our_prevotes: HashMap<Round, Prevote>,
    our_precommits: HashMap<Round, Precommit>,
    our_connect_message: Connect,

    // Информация о состоянии наших запросов
    requests: HashMap<RequestData, RequestState>,

    // Максимальная высота, на которой
    // "засветились" другие валидаторы
    validator_heights: Vec<Height>,

    // FIXME: temp, to remove
    pub commited_txs: u64,
}

// Данные, которые нас интересуют,
// специфичны для некоторой высоты
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestData {
    Propose(Hash),
    Transactions(Hash),
    Prevotes(Round, Hash),
    Precommits(Round, Hash, Hash),
    Block(Height),
}

// Состояние запроса
struct RequestState {
    // К-во попыток, которые уже произошли
    retries: u16,
    // Узлы, которые имеют интересующую нас информацию
    known_nodes: HashSet<PublicKey>,
}

pub struct ProposeState {
    /// Хеш предложения
    hash: Hash,
    // Тело предложения
    propose: Propose,
    // Множество неизвестных транзакций из этого предложения
    // FIXME: use HashSet here
    unknown_txs: BTreeSet<Hash>,
}

#[derive(Clone)]
pub struct BlockState {
    // Хэш блока
    hash: Hash,
    // Набор изменений, которые нужно внести в состояние для применения блока
    patch: Patch,
    // Хэши транзакций, закомиченных в этот блок
    txs: Vec<Hash>,
    // Раунд на котором был создан Propose
    propose_round: Round,
}

impl RequestData {
    pub fn timeout(&self) -> Duration {
        #![cfg_attr(feature="clippy", allow(match_same_arms))]
        let ms = match *self {
            RequestData::Propose(..) => REQUEST_PROPOSE_WAIT,
            RequestData::Transactions(..) => REQUEST_TRANSACTIONS_WAIT,
            RequestData::Prevotes(..) => REQUEST_PREVOTES_WAIT,
            RequestData::Precommits(..) => REQUEST_PRECOMMITS_WAIT,
            RequestData::Block(..) => REQUEST_BLOCK_WAIT,
        };
        Duration::milliseconds(ms as i64)
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

impl<Tx> State<Tx> {
    pub fn new(id: u32,
               validators: Vec<PublicKey>,
               connect: Connect,
               last_hash: Hash,
               last_height: u64)
               -> State<Tx> {
        let validators_len = validators.len();

        State {
            id: id,

            peers: HashMap::new(),
            connections: HashMap::new(),
            validators: validators,
            height: last_height,
            round: 0,
            locked_round: 0,
            locked_propose: None,
            last_hash: last_hash,

            proposes: HashMap::new(),
            blocks: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),

            transactions: HashMap::new(),
            queued: Vec::new(),

            unknown_txs: HashMap::new(),
            unknown_proposes_with_precommits: HashMap::new(),

            validator_heights: vec![0; validators_len],

            our_prevotes: HashMap::new(),
            our_precommits: HashMap::new(),
            our_connect_message: connect,

            requests: HashMap::new(),

            commited_txs: 0,
        }
    }

    pub fn id(&self) -> ValidatorId {
        self.id
    }

    // TODO Move to blockchain (and store therein)
    pub fn validators(&self) -> &[PublicKey] {
        &self.validators
    }

    pub fn add_peer(&mut self, pubkey: PublicKey, msg: Connect) -> bool {
        self.connections.insert(msg.addr(), pubkey);
        self.peers.insert(pubkey, msg).is_none()
    }

    pub fn remove_peer_with_addr(&mut self, addr: &SocketAddr) -> bool {
        if let Some(pubkey) = self.connections.remove(addr) {
            self.peers.remove(&pubkey);
            return self.validators.contains(&pubkey);
        }
        false
    }

    pub fn peers(&self) -> &HashMap<PublicKey, Connect> {
        &self.peers
    }

    pub fn public_key_of(&self, id: ValidatorId) -> Option<&PublicKey> {
        self.validators.get(id as usize)
    }

    pub fn leader(&self, round: Round) -> ValidatorId {
        ((self.height() + round as u64) % (self.validators.len() as u64)) as ValidatorId
    }

    pub fn validator_height(&self, id: ValidatorId) -> Height {
        self.validator_heights[id as usize]
    }

    pub fn set_validator_height(&mut self, id: ValidatorId, height: Height) {
        self.validator_heights[id as usize] = height;
    }

    pub fn validator_heights(&self) -> Vec<ValidatorId> {
        self.validator_heights
            .iter()
            .enumerate()
            .filter(|&(_, h)| *h > self.height())
            .map(|(v, _)| v as ValidatorId)
            .collect()
    }

    pub fn majority_count(&self) -> usize {
        // FIXME: What if validators count < 4?
        self.validators.len() * 2 / 3 + 1
    }

    pub fn height(&self) -> u64 {
        self.height
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

    pub fn propose(&mut self, hash: &Hash) -> Option<&mut ProposeState> {
        self.proposes.get_mut(hash)
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
    pub fn new_height(&mut self, block_hash: &Hash, round: Round) {
        self.height += 1;
        self.round = round;
        self.locked_round = 0;
        self.locked_propose = None;
        self.last_hash = *block_hash;
        {
            // Commit transactions if needed
            let txs = self.block(block_hash).unwrap().txs.clone();
            for hash in txs {
                if self.transactions.remove(&hash).is_some() {
                    self.commited_txs += 1;
                }
            }
        }
        // TODO: destruct/construct structure HeightState instead of call clear
        self.blocks.clear();
        self.proposes.clear();
        self.prevotes.clear();
        self.precommits.clear();
        self.our_prevotes.clear();
        self.our_precommits.clear();
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

    pub fn transactions(&self) -> &HashMap<Hash, Tx> {
        &self.transactions
    }

    pub fn add_transaction(&mut self, hash: Hash, msg: Tx) -> Vec<(Hash, Round)> {
        let mut full_proposes = Vec::new();
        for (hash, state) in &mut self.proposes {
            state.unknown_txs.remove(hash);
            if state.unknown_txs.is_empty() {
                full_proposes.push((*hash, state.message().round()));
            }
        }
        self.transactions.insert(hash, msg);
        full_proposes
    }

    pub fn prevotes(&self,
                    round: Round,
                    propose_hash: Hash)
                    -> Option<&HashMap<ValidatorId, Prevote>> {
        self.prevotes.get(&(round, propose_hash))
    }

    pub fn precommits(&self,
                      round: Round,
                      block_hash: Hash)
                      -> Option<&HashMap<ValidatorId, Precommit>> {
        self.precommits.get(&(round, block_hash))
    }

    pub fn add_self_propose(&mut self, msg: Propose) -> Hash {
        debug_assert_eq!(msg.validator(), self.id);
        let propose_hash = msg.hash();
        self.proposes.insert(propose_hash,
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
                    .collect(): BTreeSet<Hash>;
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
        if msg.validator() == self.id() {
            if let Some(other) = self.our_prevotes.insert(msg.round(), msg.clone()) {
                if &other != msg {
                    panic!("Trying to send different prevotes for same round, old={:?}, new={:?}",
                           other,
                           msg);
                }
            }
        }

        let key = (msg.round(), *msg.propose_hash());
        let map = self.prevotes.entry(key).or_insert_with(HashMap::new);
        map.entry(msg.validator()).or_insert_with(|| msg.clone());

        map.len() >= majority_count
    }

    pub fn has_majority_prevotes(&self, round: Round, propose_hash: Hash) -> bool {
        match self.prevotes.get(&(round, propose_hash)) {
            Some(map) => map.len() >= self.majority_count(),
            None => false,
        }
    }

    // TODO implement and use Votes data structure
    pub fn has_prevotes(&self, round: Round, propose_hash: &Hash) -> BitVec {
        let mut vec = BitVec::from_elem(self.validators.len(), false);
        if let Some(map) = self.prevotes.get(&(round, *propose_hash)) {
            for prevote in map.values() {
                vec.set(prevote.validator() as usize, true);
            }
        }
        vec
    }

    pub fn has_precommits(&self, round: Round, propose_hash: &Hash) -> BitVec {
        let mut vec = BitVec::from_elem(self.validators.len(), false);
        if let Some(map) = self.precommits.get(&(round, *propose_hash)) {
            for precommit in map.values() {
                vec.set(precommit.validator() as usize, true);
            }
        }
        vec
    }

    pub fn add_precommit(&mut self, msg: &Precommit) -> bool {
        let majority_count = self.majority_count();
        if msg.validator() == self.id() {
            if let Some(other) = self.our_precommits.insert(msg.round(), msg.clone()) {
                if other.propose_hash() != msg.propose_hash() {
                    panic!("Trying to send different precommits for same round, old={:?}, \
                            new={:?}",
                           other,
                           msg);
                }
            }
        }

        let key = (msg.round(), *msg.block_hash());
        let map = self.precommits.entry(key).or_insert_with(HashMap::new);
        map.entry(msg.validator()).or_insert_with(|| msg.clone());

        map.len() >= majority_count
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
        self.unknown_proposes_with_precommits.remove(propose_hash).unwrap_or_default()
    }

    pub fn has_majority_precommits(&self, round: Round, block_hash: Hash) -> bool {
        match self.precommits.get(&(round, block_hash)) {
            Some(map) => map.len() >= self.majority_count(),
            None => false,
        }
    }

    pub fn have_prevote(&self, round: Round) -> bool {
        self.our_prevotes.get(&round).is_some()
    }

    pub fn have_incompatible_prevotes(&self) -> bool {
        for round in self.locked_round + 1...self.round {
            if let Some(msg) = self.our_prevotes.get(&round) {
                // TODO: unefficient
                if Some(*msg.propose_hash()) != self.locked_propose {
                    return true;
                }
            }
        }
        false
    }

    pub fn request(&mut self, data: RequestData, peer: PublicKey) -> bool {
        let mut state = self.requests
            .entry(data)
            .or_insert_with(RequestState::new);
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
