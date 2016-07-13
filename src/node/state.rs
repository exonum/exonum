use std::net::SocketAddr;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;

use time::{Timespec, Duration, get_time};

const REQUEST_PROPOSE_WAIT       : u64 = 1_000_000; // milliseconds
const REQUEST_TRANSACTIONS_WAIT  : u64 = 1_000_000;
const REQUEST_PREVOTES_WAIT      : u64 = 1_000_000;
const REQUEST_PRECOMMITS_WAIT    : u64 = 1_000_000;
const REQUEST_COMMIT_WAIT        : u64 = 1_000_000;
const REQUEST_PEERS_WAIT         : u64 = 1_000_000;

use super::super::messages::{
    TxMessage, Message, RequestMessage,
    Propose, Prevote, Precommit, ConsensusMessage
};
use super::super::crypto::{PublicKey, Hash, hash};
use super::super::storage::{Patch};

pub type Round = u32;
pub type Height = u64;
pub type ValidatorId = u32;

// TODO: reduce copying of Hash

pub struct State {
    id: u32,
    peers: HashMap<PublicKey, SocketAddr>,
    validators: Vec<PublicKey>,
    height: u64,
    round: Round,
    locked_round: Round,
    locked_propose: Option<Hash>,

    // messages
    proposes: HashMap<Hash, ProposeState>,
    prevotes: HashMap<(Round, Hash), HashMap<ValidatorId, Prevote>>,
    precommits: HashMap<(Round, Hash, Hash), HashMap<ValidatorId, Precommit>>,

    transactions: HashMap<Hash, TxMessage>,
    queued: Vec<ConsensusMessage>,

    unknown_txs: HashMap<Hash, Vec<Hash>>,
    unknown_proposes_with_precommits: HashMap<Hash, Vec<(Round, Hash)>>,

    // TODO: add hashmap of transactions we wait for

    our_prevotes: HashMap<Round, Prevote>,
    our_precommits: HashMap<Round, Precommit>,

    // Информация о состоянии наших запросов
    requests: HashMap<RequestData, RequestState>,

    // Максимальная высота, на которой
    // "засветились" другие валидаторы
    validator_heights: Vec<Height>,
}

// Данные, которые нас интересуют,
// специфичны для некоторой высоты
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum RequestData {
    Propose(Hash),
    Transactions(Hash),
    Prevotes(Round, Hash),
    Precommits(Round, Hash, Hash),
    Commit, // TODO: add height?
    Peers
}

// Состояние запроса
struct RequestState {
    // К-во попыток, которые уже произошли
    retries: u16,
    // Узлы, которые имеют интересующую нас информацию
    known_nodes: HashSet<ValidatorId>
}

pub struct ProposeState {
    // Тело предложения
    propose:        Propose,
    // Вычисленный хеш блока (из предложения)
    block_hash:     Option<Hash>,
    // Набор изменений, которые нужно внести в состояние для применения блока
    patch:          Option<Patch>,
    // Множество неизвестных транзакций из этого предложения
    unknown_txs:    HashSet<Hash>
}

impl RequestData {
    pub fn timeout(&self) -> Timespec { // TODO: return Duration
        let ms = match *self {
            RequestData::Propose(..)      => REQUEST_PROPOSE_WAIT,
            RequestData::Transactions(..) => REQUEST_TRANSACTIONS_WAIT,
            RequestData::Prevotes(..)     => REQUEST_PREVOTES_WAIT,
            RequestData::Precommits(..)   => REQUEST_PRECOMMITS_WAIT,
            RequestData::Commit           => REQUEST_COMMIT_WAIT,
            RequestData::Peers            => REQUEST_PEERS_WAIT,
        };
        get_time() + Duration::milliseconds(ms as i64)
    }
}

impl RequestState {
    fn new() -> RequestState {
        RequestState {
            retries: 0,
            known_nodes: HashSet::new()
        }
    }

    fn insert(&mut self, validator: ValidatorId) {
        self.known_nodes.insert(validator);
    }

    fn remove(&mut self, validator: &ValidatorId) {
        self.retries += 1;
        self.known_nodes.remove(validator);
    }

    fn is_empty(&mut self) -> bool {
        self.known_nodes.is_empty()
    }

    fn peek(&self) -> Option<ValidatorId> {
        self.known_nodes.iter().next().map(|v| *v)
    }
}

impl ProposeState {
    pub fn message(&self) -> &Propose {
        &self.propose
    }

    pub fn unknown_txs(&self) -> &HashSet<Hash> {
        &self.unknown_txs
    }

    pub fn has_unknown_txs(&self) -> bool {
        !self.unknown_txs.is_empty()
    }

    pub fn block_hash(&self) -> Option<Hash> {
        self.block_hash
    }
}

impl State {
    pub fn new(id: u32,
               validators: Vec<PublicKey>) -> State {
        let validators_len = validators.len() as u64;

        State {
            id: id,

            peers: HashMap::new(),
            validators: validators,
            height: 0,
            round: 1,
            locked_round: 0,
            locked_propose: None,

            proposes: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),

            transactions: HashMap::new(),
            queued: Vec::new(),

            unknown_txs: HashMap::new(),
            unknown_proposes_with_precommits: HashMap::new(),

            validator_heights: vec![0, validators_len],

            our_prevotes: HashMap::new(),
            our_precommits: HashMap::new(),

            requests: HashMap::new(),
        }
    }

    pub fn id(&self) -> ValidatorId {
        self.id
    }

    pub fn validators(&self) -> &[PublicKey] {
        &self.validators
    }

    pub fn add_peer(&mut self, pubkey: PublicKey, addr: SocketAddr) -> bool {
        self.peers.insert(pubkey, addr).is_none()
    }

    pub fn peers(&self)
            -> &HashMap<PublicKey, SocketAddr> {
        &self.peers
    }

    pub fn public_key_of(&self, id: ValidatorId) -> Option<&PublicKey> {
        self.validators.get(id as usize)
    }

    pub fn leader(&self, round: Round) -> ValidatorId {
        ((self.height() + round as u64) %
         (self.validators.len() as u64)) as ValidatorId
    }

    pub fn validator_height(&self, id: ValidatorId) -> Height {
        self.validator_heights[id as usize]
    }

    pub fn set_validator_height(&mut self, id: ValidatorId, height: Height) {
        self.validator_heights[id as usize] = height;
    }

    pub fn validator_heights(&self) -> Vec<ValidatorId> {
        self.validator_heights.iter()
                              .enumerate()
                              .filter(|&(v, h)| *h > self.height())
                              .map(|(v, h)| v as ValidatorId)
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

    pub fn new_round(&mut self) {
        self.round += 1;
    }

    pub fn new_height(&mut self, propose_hash: &Hash) {
        self.height += 1;
        self.round = 1;
        self.locked_round = 0;
        // TODO: destruct/construct structure HeightState instead of call clear
        self.proposes.clear();
        self.prevotes.clear();
        self.precommits.clear();
        self.our_prevotes.clear();
        self.our_precommits.clear();
        self.requests.clear(); // FIXME: clear all timeouts

        let state = self.proposes.get(&propose_hash)
                                 .expect("Trying to commit unknown propose");
        for tx in state.propose.transactions() {
            self.transactions.remove(tx);
        }
    }

    pub fn queued(&mut self) -> Vec<ConsensusMessage> {
        let mut queued = Vec::new();
        ::std::mem::swap(&mut self.queued, &mut queued);
        queued
    }

    pub fn add_queued(&mut self, msg: ConsensusMessage) {
        self.queued.push(msg);
    }

    pub fn transactions(&self) -> &HashMap<Hash, TxMessage> {
        &self.transactions
    }

    pub fn add_transaction(&mut self, hash: Hash, msg: TxMessage) -> Vec<Hash> {
        let mut full_proposes = Vec::new();
        for (hash, state) in self.proposes.iter_mut() {
            state.unknown_txs.remove(&hash);
            if state.unknown_txs.is_empty() {
                full_proposes.push(*hash);
            }
        }
        self.transactions.insert(hash, msg);
        return full_proposes;
    }

    pub fn prevotes(&self, round: Round, propose_hash: Hash)
        -> Option<&HashMap<ValidatorId, Prevote>> {
        self.prevotes.get(&(round, propose_hash))
    }

    pub fn precommits(&self, round: Round, propose_hash: Hash, block_hash: Hash)
        -> Option<&HashMap<ValidatorId, Precommit>> {
        self.precommits.get(&(round, propose_hash, block_hash))
    }

    pub fn add_propose(&mut self, propose_hash: Hash, msg: &Propose) -> bool {
        let txs = &self.transactions;
        match self.proposes.entry(propose_hash) {
            Entry::Occupied(..) => false,
            Entry::Vacant(e) => {
                let unknown_txs = msg.transactions()
                                     .iter()
                                     .filter(|tx| !txs.contains_key(tx))
                                     .map(|tx| *tx)
                                     .collect(): HashSet<Hash>;
                for tx in &unknown_txs {
                    self.unknown_txs.entry(*tx)
                                    .or_insert_with(Vec::new)
                                    .push(propose_hash);
                };
                e.insert(ProposeState {
                    propose: msg.clone(),
                    block_hash: None,
                    patch: None,
                    unknown_txs: unknown_txs
                });
                true
            }
        }
    }

    pub fn add_prevote(&mut self, msg: &Prevote) -> bool {
        let majority_count = self.majority_count();
        if msg.validator() == self.id() {
            if let Some(_) = self.our_prevotes.insert(msg.round(), msg.clone()) {
                panic!("Trying to send different prevotes for same round");
            }
        }

        let key = (msg.round(), *msg.propose_hash());
        let map = self.prevotes.entry(key).or_insert_with(|| HashMap::new());
        map.entry(msg.validator()).or_insert_with(|| msg.clone());

        map.len() >= majority_count
    }

    pub fn has_majority_prevotes(&self, round: Round, propose_hash: Hash) -> bool {
        match self.prevotes.get(&(round, propose_hash)) {
            Some(map) => map.len() >= self.majority_count(),
            None => false
        }
    }

    pub fn add_precommit(&mut self, msg: &Precommit) -> bool {
        let majority_count = self.majority_count();
        if msg.validator() == self.id() {
            if let Some(_) = self.our_precommits.insert(msg.round(), msg.clone()) {
                panic!("Trying to send different precommits for same round");
            }
        }

        let key = (msg.round(), *msg.propose_hash(), *msg.block_hash());
        let map = self.precommits.entry(key).or_insert_with(|| HashMap::new());
        map.entry(msg.validator()).or_insert_with(|| msg.clone());

        map.len() >= majority_count
    }

    pub fn add_unknown_propose_with_precommits(&mut self,
                                               round: Round,
                                               propose_hash: Hash,
                                               block_hash: Hash) {
        self.unknown_proposes_with_precommits.entry(propose_hash)
                                             .or_insert_with(Vec::new)
                                             .push((round, block_hash));
    }

    pub fn unknown_propose_with_precommits(&mut self, propose_hash: &Hash)
        -> Vec<(Round, Hash)> {
        self.unknown_proposes_with_precommits.remove(propose_hash).unwrap_or_default()
    }

    pub fn has_majority_precommits(&self,
                                   round: Round,
                                   propose_hash: Hash,
                                   block_hash: Hash) -> bool {
        match self.precommits.get(&(round, propose_hash, block_hash)) {
            Some(map) => map.len() >= self.majority_count(),
            None => false
        }
    }

    pub fn have_prevote(&self, round: Round) -> bool {
        self.our_prevotes.get(&round).is_some()
    }

    pub fn have_incompatible_prevotes(&self) -> bool {
        for round in self.locked_round + 1 ... self.round {
            match self.our_prevotes.get(&round) {
                Some(msg) => {
                    // TODO: unefficient
                    if Some(*msg.propose_hash()) != self.locked_propose {
                        return true
                    }
                },
                None => (),
            }
        }
        false
    }

    pub fn request(&mut self, data: RequestData, validator: ValidatorId) -> bool {
        let mut state = self.requests.entry(data)
                                     .or_insert_with(RequestState::new);
        let is_new = state.is_empty();
        state.insert(validator);
        return is_new;
    }

    pub fn retry(&mut self, data: &RequestData, validator: ValidatorId) -> Option<ValidatorId> {
        let next = {
            let mut state = self.requests.get_mut(data).unwrap();
            state.remove(&validator);
            state.peek()
        };

        if next.is_none() {
            self.requests.remove(data);
        };

        return next;
    }

    pub fn remove_request(&mut self, data: &RequestData) -> HashSet<ValidatorId> {
        let state = self.requests.remove(data);
        state.map(|s| s.known_nodes).unwrap_or_default()
    }
}
