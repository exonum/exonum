use std::net::SocketAddr;
use std::collections::HashMap;

use time::{Timespec, get_time};

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
    proposals: HashMap<Hash, Propose>,
    prevotes: HashMap<(Round, Hash), HashMap<ValidatorId, Prevote>>,
    precommits: HashMap<(Round, Hash, Hash), HashMap<ValidatorId, Precommit>>,

    transactions: HashMap<Hash, TxMessage>,
    queued: Vec<ConsensusMessage>,

    patches: HashMap<Hash, Patch>,

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
enum RequestData {
    Propose(Hash),
    Transactions(Hash),
    Prevotes(Round, Hash),
    Precommits(Round, Hash, Hash),
    Commit,
    Peers
}

// Состояние запроса
struct RequestState {
    // К-во попыток, которые уже произошли
    retries: u16,
    // Наше сообщение, сформированное и подписанное
    message: RequestMessage,
    // Узлы, которые имеют интересующую нас информацию
    known_nodes: Vec<ValidatorId>
}

// Таймаут, который сигнализирует о необходимости
// отправить запрос
struct RequestWaitTimeout {
    // Высота, которой соответсвует данный таймаут
    height: u64,
    // Данные, который мы должны запросить
    data: RequestData
}

// Таймаут, который сигнализирует о необходимости
// отправить запрос еще раз
struct RequestRetryTimeout {
    // Высота, которой соответсвует данный таймаут
    height: u64,
    // Номер попытки, которая завершилась таймаутом
    retry: u16,
    // Данные, который мы должны запросить
    data: RequestData,
    // Валидатор, у которого мы запрашивали данные
    validator_id: ValidatorId
}

impl State {
    pub fn new(id: u32,
               validators: Vec<PublicKey>) -> State {
        State {
            id: id,

            peers: HashMap::new(),
            validators: validators,
            height: 0,
            round: 1,
            locked_round: 0,
            locked_propose: None,

            proposals: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),

            transactions: HashMap::new(),
            queued: Vec::new(),

            patches: HashMap::new(),

            our_prevotes: HashMap::new(),
            our_precommits: HashMap::new(),
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

    pub fn proposal(&self, hash: &Hash) -> Option<&Propose> {
        self.proposals.get(hash)
    }

    pub fn new_round(&mut self) {
        self.round += 1;
    }

    pub fn new_height(&mut self, propose_hash: &Hash) {
        self.height += 1;
        self.round = 1;
        self.locked_round = 0;
        // TODO: destruct/construct structure HeightState instead of call clear
        self.proposals.clear();
        self.prevotes.clear();
        self.precommits.clear();
        self.our_prevotes.clear();
        self.our_precommits.clear();
        self.patches.clear();

        let propose = self.proposals.get(&propose_hash)
                                    .expect("Trying to commit unknown proposal");
        for tx in propose.transactions() {
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

    pub fn add_transaction(&mut self, hash: Hash, msg: TxMessage) {
        self.transactions.insert(hash, msg);
    }

    pub fn patch(&self, hash: &Hash) -> Option<&Patch> {
        self.patches.get(hash)
    }

    pub fn set_patch(&mut self, hash: Hash, patch: Patch) -> &Patch {
        self.patches.entry(hash).or_insert(patch)
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
        self.proposals.insert(propose_hash, msg.clone()).is_none()
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
}
