use std::{collections, net};

use super::signature::PublicKey;

pub struct State {
    validators: collections::HashMap<PublicKey, net::SocketAddr>,
    height: usize,
    prevotes: usize,
}

impl State {
    pub fn new() -> State {
        State {
            validators: collections::HashMap::new(),
            height: 0,
            prevotes: 0
        }
    }

    pub fn add_validator(&mut self,
                         public_key: PublicKey, address: net::SocketAddr) {
        self.validators.insert(public_key, address);
    }

    pub fn validate_height(&self, new_height: usize) -> bool {
        self.height + 1 == new_height
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn new_height(&mut self, new_height: usize) {
        self.height = new_height;
        self.prevotes = 0
    }

    pub fn validators(&self)
            -> &collections::HashMap<PublicKey, net::SocketAddr> {
        &self.validators
    }

    pub fn has_consensus(&self) -> bool {
        self.prevotes >= 2
    }

    pub fn add_prevote(&mut self) {
        self.prevotes += 1
    }
}
