use std::{collections, net};

use time::{Timespec, get_time};

use super::message::Message;
use super::crypto::{PublicKey, Hash, hash};

pub struct State {
    peers: collections::HashMap<PublicKey, net::SocketAddr>,
    validators: Vec<PublicKey>,
    height: u64,
    round: u32,
    rounds: Vec<RoundState>,
    prev_hash: Hash,
    prev_time: Timespec,
    checkpoint_time: Timespec,
    locked_round: u32,
    queue: Vec<Message>,
}

pub enum RoundState {
    KnownProposal(ProposalState),
    UnknownProposal(Vec<Message>)
}

pub struct ProposalState {
    hash: Hash,
    propose: Message,
    prevotes: collections::HashMap<PublicKey, Message>,
    precommits: collections::HashMap<PublicKey, Message>,
}

impl ProposalState {
    fn new(propose: Message) -> ProposalState {
        ProposalState {
            hash: propose.hash(),
            propose: propose,
            prevotes: collections::HashMap::new(),
            precommits: collections::HashMap::new(),
        }
    }
}

impl State {
    pub fn new(validators: Vec<PublicKey>) -> State {
        State {
            peers: collections::HashMap::new(),
            validators: validators,
            height: 0,
            round: 1,
            rounds: Vec::new(),
            // TODO: use genesis block instead
            prev_hash: super::crypto::hash(&[]),
            prev_time: get_time(),
            checkpoint_time: get_time(),
            locked_round: 0,
            queue: Vec::new()
        }
    }

    pub fn add_peer(&mut self,
                    public_key: PublicKey, address: net::SocketAddr) -> bool {
        self.peers.insert(public_key, address).is_none()
    }

    pub fn peers(&self)
            -> &collections::HashMap<PublicKey, net::SocketAddr> {
        &self.peers
    }

    pub fn leader(&self, round: u32) -> &PublicKey {
        let idx = self.height() as u64 + round as u64;
        &self.validators[(idx % 2) as usize]
    }

    pub fn consensus_count(&self) -> usize {
        // FIXME: temporary constant
        // FIXME: What if validators count < 4?
        2
        // self.validators.len() * 2 / 3 + 1
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    pub fn round(&self) -> u32 {
        self.round
    }

    pub fn prev_hash(&self) -> &Hash {
        &self.prev_hash
    }

    pub fn prev_time(&self) -> Timespec {
        self.prev_time
    }

    pub fn round_state(&mut self, round: u32) -> &mut RoundState {
        while self.rounds.len() < round as usize {
            self.rounds.push(RoundState::UnknownProposal(Vec::new()));
        }
        &mut self.rounds[round as usize - 1]
    }

    pub fn lock_round(&mut self, round: u32) {
        self.locked_round = round;
    }

    pub fn new_round(&mut self) {
        self.round += 1;
    }

    pub fn new_height(&mut self, hash: Hash) -> Vec<Message> {
        self.height += 1;

        if self.height % 250 == 0 {
            let now = get_time();
            let bps = 250_000f64 / (now - self.checkpoint_time).num_milliseconds() as f64;
            info!("Commit {} blocks per second (height {})", bps as u32, self.height);
            self.checkpoint_time = now;
        }

        self.round = 1;
        self.prev_hash = hash;
        self.prev_time = get_time();
        self.locked_round = 0;
        self.rounds.clear();
        let mut queue = Vec::new();
        ::std::mem::swap(&mut self.queue, &mut queue);
        queue
    }

    pub fn queue(&mut self, message: Message) {
        self.queue.push(message);
    }

    pub fn add_propose(&mut self,
                       round: u32,
                       message: Message) -> (Hash, Vec<Message>) {
        let proposal_state = ProposalState::new(message);
        let hash = proposal_state.hash.clone();
        let mut state = RoundState::KnownProposal(proposal_state);
        ::std::mem::swap(self.round_state(round), &mut state);
        match state {
            RoundState::KnownProposal(_) => {
                // FIXME: double proposal attack
                panic!("Double proposal attack");
            }
            RoundState::UnknownProposal(queue) => (hash, queue)
        }
    }

    pub fn add_prevote(&mut self,
                       round: u32,
                       hash: &Hash,
                       message: Message) -> bool {
        let cc = self.consensus_count();
        let locked_round = self.locked_round;
        match *self.round_state(round) {
            RoundState::KnownProposal(ref mut state) => {
                if state.hash != *hash {
                    return false;
                }
                state.prevotes.insert(message.public_key().clone(), message);
                state.prevotes.len() >= cc && locked_round < round
            },
            RoundState::UnknownProposal(ref mut queue) => {
                queue.push(message);
                false
            }
        }
    }

    pub fn add_precommit(&mut self,
                         round: u32,
                         hash: &Hash,
                         message: Message) -> bool {
        let cc = self.consensus_count();
        match *self.round_state(round) {
            RoundState::KnownProposal(ref mut state) => {
                if state.hash != *hash {
                    return false;
                }
                state.precommits.insert(message.public_key().clone(), message);
                state.precommits.len() >= cc
            },
            RoundState::UnknownProposal(ref mut queue) => {
                queue.push(message);
                false
            }
        }
    }
}
