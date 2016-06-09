use std::net::SocketAddr;
use std::collections::HashMap;

use time::{Timespec, get_time};

use super::super::messages::{Propose, Prevote, Precommit, ConsensusMessage, TxMessage, Message};
use super::super::crypto::{PublicKey, Hash, hash};
use super::super::storage::{Changes};

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
    locked_propose: Hash,
    prev_hash: Hash,

    // messages
    proposals: HashMap<Hash, Propose>,
    prevotes: HashMap<(Round, Hash), HashMap<ValidatorId, Prevote>>,
    precommits: HashMap<(Round, Hash, Hash), HashMap<ValidatorId, Precommit>>,

    transactions: HashMap<Hash, TxMessage>,
    queued: Vec<ConsensusMessage>,

    changes: HashMap<Hash, Changes>,

    // TODO: add hashmap of transactions we wait for

    our_prevotes: HashMap<Round, Prevote>,
    our_precommits: HashMap<Round, Precommit>
}

impl State {
    pub fn new(id: u32,
               validators: Vec<PublicKey>,
               prev_hash: Hash) -> State {
        State {
            peers: HashMap::new(),
            validators: validators,
            height: 0,
            round: 1,
            locked_round: 0,
            prev_hash: Hash,

            proposals: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),

            transactions: HashMap::new(),
            queued: Vec::new(),

            changes: HashMap::new(),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn add_peer(&mut self, pubkey: PublicKey, addr: SocketAddr) -> bool {
        self.peers.insert(pubkey, addr).is_none()
    }

    pub fn peers(&self)
            -> &HashMap<PublicKey, SocketAddr> {
        &self.peers
    }

    pub fn public_key_of(&self, id: ValidatorId) -> Option<PublicKey> {
        self.validators.get(id)
    }

    pub fn leader(&self, round: u32) -> ValidatorId {
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
        self.locked_propose = hash;
    }

    pub fn new_round(&mut self) {
        self.round += 1;
    }

    pub fn new_height(&mut self, hash: Hash) {
        self.height += 1;
        self.round = 1;
        self.locked_round = 0;
        // TODO: destruct/construct structure HeightState instead of call clear
        self.proposals.clear();
        self.prevotes.clear();
        self.precommits.clear();
        self.our_prevotes.clear();
        self.our_precommits.clear();
        self.changes.clear();

        let propose = self.proposals.get(hash)
                                    .except("Trying to commit unknown proposal");
        for tx in propose.transactions() {
            self.transactions.remove(tx)
        }
    }

    pub fn queued(&mut self) -> Vec<ConsensusMessage> {
        let mut queue = Vec::new();
        ::std::mem::swap(&mut self.queue, &mut queue);
        queue
    }

    pub fn add_queued(&mut self, msg: ConsensusMessage) {
        self.queue.push(msg);
    }

    pub fn transactions(&self) -> Vec<Hash> {
        self.transactions.keys().collect()
    }

    pub fn add_transaction(&mut self, hash: Hash, msg: TxMessage) {
        self.tx_pool.insert(hash, msg);
    }

    pub fn state_hash(&self, hash: Hash) -> Option<Hash> {
        self.changes.get(hash).map(|changes| changes.state_hash())
    }

    pub fn add_changes(&mut self, hash: Hash, changes: Changes) {
        self.changes.insert(hash, changes);
    }

    pub fn add_propose(&mut self, hash: Hash, msg: &Propose) -> bool {
        self.proposes.insert(hash, msg) == None
    }

    pub fn add_prevote(&mut self, msg: &Prevote) -> bool {
        if msg.validator() == self.id() {
            if let Some(prev) = self.our_prevotes.insert(msg) {
                if prev != msg {
                    panic!("Trying to send different prevotes for same round");
                }
            }
        }

        let key = (msg.round(), msg.block_hash());
        let map = self.prevotes.entry(key).or_insert_with(|| HashMap::new());
        map.entry(msg.validator()).or_insert_with(|| msg.clone());

        map.len() >= self.majority_count()
    }

    pub fn has_majority_prevotes(&self, round: Round, hash: Hash) -> bool {
        match self.prevotes.get((round, hash)) {
            Some(map) => map.len() >= self.majority_count(),
            None => false
        }
    }

    pub fn add_precommit(&mut self, msg: &Precommit) -> bool {
        if msg.validator() == self.id() {
            if let Some(prev) = self.our_precommits.insert(msg) {
                if prev != msg {
                    panic!("Trying to send different precommits for same round");
                }
            }
        }

        let key = (msg.round(), msg.block_hash(), msg.state_hash());
        let map = self.precommits.entry(key).or_insert_with(|| HashMap::new());
        map.entry(msg.validator()).or_insert_with(|| msg.clone());

        map.len() >= self.majority_count()
    }

    pub fn has_majority_precommits(&self, round: Round, block_hash: Hash,
                                   state_hash: Hash) -> bool {
        match self.precommits.get((round, block_hash, state_hash)) {
            Some(map) => map.len() >= self.majority_count(),
            None => false
        }
    }

    pub fn have_incompatible_prevotes(&self) -> bool {
        for round in self.locked_round + 1 ... self.round {
            match self.our_prevotes.get(round) {
                Some(msg) => {
                    if msg.block_hash() != self.locked_propose {
                        return true
                    }
                },
                None => (),
            }
        }
        false
    }
}
