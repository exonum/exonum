use std::net::SocketAddr;
use std::collections::HashMap;

use time::{Timespec, get_time};

use super::super::messages::{Propose, Prevote, Precommit, ConsensusMessage, TxMessage, Message};
use super::super::crypto::{PublicKey, Hash, hash};

type Round = u32;
type ValidatorId = u32;

// TODO: reduce copying of Hash

pub struct State {
    peers: HashMap<PublicKey, SocketAddr>,
    validators: Vec<PublicKey>,
    height: u64,
    round: Round,
    locked_round: Round,
    locked_propose: Hash,
    prev_hash: Hash,

    // messages
    proposals: HashMap<Hash, Proposal>,
    prevotes: HashMap<(Round, Hash), HashMap<ValidatorId, Prevote>>,
    precommits: HashMap<(Round, Hash, Hash), HashMap<ValidatorId, Precommit>>,

    transactions: HashMap<Hash, TxMessage>,
    queued: Vec<ConsensusMessage>,

    changes: HashMap<Hash, Changes>,

    // TODO: add hashmap of transactions we wait for
}

impl State {
    pub fn new(validators: Vec<PublicKey>, prev_hash: Hash) -> State {
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

    pub fn lock(&mut self, hash: Hash) {
        self.locked_round = self.round;
        self.locked_propose = hash;
    }

    pub fn new_round(&mut self) {
        self.round += 1;
    }

    pub fn new_height(&mut self, hash: Hash) {
        self.height += 1;
        self.round = 1;
        self.locked_round = 0;
        self.proposals.clear();
        self.prevotes.clear();
        self.precommits.clear();
        self.changes.clear()
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

    pub fn add_propose(&mut self, msg: &Propose) -> Hash {
        let hash = msg.hash();
        self.proposes.entry(&hash).or_insert_with(|| msg.clone());
        hash
    }

    pub fn add_prevote(&mut self, msg: &Prevote) -> bool {
        let key = (msg.round(), msg.block_hash());
        let map = self.prevotes.entry(key).or_insert_with(|| HashMap::new());
        map.entry(msg.validator()).or_insert_with(|| msg.clone());

        if self.locked_round >= msg.round() {
            return false
        }
        map.len() >= self.majority_count()
    }

    pub fn has_majority_prevotes(&self, round: Round, hash: Hash) -> bool {
        if self.locked_round >= round {
            return false
        }
        match self.prevotes.get((round, hash)) {
            Some(map) => map.len() >= self.majority_count(),
            None => false
        }
    }

    pub fn add_precommit(&mut self, msg: &Precommit) -> bool {
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
}
