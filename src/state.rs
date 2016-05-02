use std::{collections, net};

use super::message::Message;
use super::crypto::PublicKey;

pub struct State {
    validators: collections::HashMap<PublicKey, net::SocketAddr>,
    height: usize,
    rounds: Vec<RoundState>,
}

pub enum RoundState {
    KnownProposal(ProposalState),
    UnknownProposal(Vec<Message>)
}

pub struct ProposalState {
    propose: Message,
    prevotes: collections::HashMap<PublicKey, Message>,
    precommits: collections::HashMap<PublicKey, Message>,
}

impl ProposalState {
    fn new(propose: Message) -> ProposalState {
        ProposalState {
            propose: propose,
            prevotes: collections::HashMap::new(),
            precommits: collections::HashMap::new(),
        }
    }
}

impl State {
    pub fn new() -> State {
        State {
            validators: collections::HashMap::new(),
            height: 0,
            rounds: Vec::new(),
        }
    }

    pub fn add_validator(&mut self,
                         public_key: PublicKey, address: net::SocketAddr) {
        self.validators.insert(public_key, address);
    }

    pub fn validators(&self)
            -> &collections::HashMap<PublicKey, net::SocketAddr> {
        &self.validators
    }

    pub fn validate_height(&self, new_height: usize) -> bool {
        self.height + 1 == new_height
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn round(&self) -> usize {
        self.rounds.len()
    }

    pub fn round_state(&mut self) -> &mut RoundState {
        self.rounds.last_mut().unwrap()
    }

    pub fn new_height(&mut self) {
        self.height += 1;
        self.rounds.clear();
        self.new_round();
    }

    pub fn new_round(&mut self) {
        self.rounds.push(RoundState::UnknownProposal(Vec::new()));
    }

    pub fn has_consensus(&self) -> bool {
        false
    }

    // pub fn add_prevote(&mut self, message: Message) {
    //     self.prevotes.insert(message.public_key(), message)
    // }

    // pub fn add_precommit(&mut self, message: Message) {
    //     self.precommits.insert(message.public_key(), message)
    // }
}
