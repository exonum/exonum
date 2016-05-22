use std::net::SocketAddr;

use time::{get_time, Duration};

use super::crypto::{PublicKey, SecretKey};
use super::events::{Events, Event, Timeout, EventsConfiguration};
use super::network::{Network, NetworkConfiguration};
use super::messages::{Any, Connect, Propose, Prevote, Precommit, Commit, RawMessage, ProtocolMessage};
use super::state::{State};

// TODO: avoid recursion calls?

pub struct Node {
    public_key: PublicKey,
    secret_key: SecretKey,
    state: State,
    events: Events,
    network: Network,
    propose_timeout: u32,
    round_timeout: u32,
    byzantine: bool,
    // TODO: move this into peer exchange service
    peer_discovery: Vec<SocketAddr>
}

#[derive(Debug)]
pub struct Configuration {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub events: EventsConfiguration,
    pub network: NetworkConfiguration,
    pub propose_timeout: u32,
    pub round_timeout: u32,
    pub peer_discovery: Vec<SocketAddr>,
    pub validators: Vec<PublicKey>,
    pub byzantine: bool,
}

impl Node {
    pub fn with_config(config: Configuration) -> Node {
        // FIXME: remove unwraps here, use FATAL log level instead
        let events = Events::with_config(config.events).unwrap();
        let network = Network::with_config(config.network);
        let state = State::new(config.validators);
        Node {
            public_key: config.public_key,
            secret_key: config.secret_key,
            state: state,
            events: events,
            network: network,
            propose_timeout: config.propose_timeout,
            round_timeout: config.round_timeout,
            peer_discovery: config.peer_discovery,
            byzantine: config.byzantine
        }
    }

    fn initialize(&mut self) {
        // info!("Start listening...");
        self.network.bind(&mut self.events).unwrap();
        let message = Connect::new(self.network.address().clone(), get_time(),
                                   &self.public_key, &self.secret_key);
        for address in self.peer_discovery.iter() {
            if address == self.network.address() {
                continue
            }
            self.network.send_to(&mut self.events,
                                 address,
                                 message.raw().clone()).unwrap();
        }

        self.add_timeout();
    }

    pub fn run(&mut self) {
        self.initialize();
        loop {
            if self.state.height() == 1000 {
                break;
            }
            match self.events.poll() {
                Event::Incoming(message) => {
                    self.handle(message, false);
                },
                Event::Internal(_) => {

                },
                Event::Timeout(timeout) => {
                    self.handle_timeout(timeout);
                },
                Event::Io(id, set) => {
                    // TODO: shoud we call network.io through main event queue?
                    // FIXME: Remove unwrap here
                    self.network.io(&mut self.events, id, set).unwrap()
                },
                Event::Error(_) => {

                },
                Event::Terminate => {
                    break
                }
            }
        }
    }

    fn handle_timeout(&mut self, timeout: Timeout) {
        if timeout.height != self.state.height() {
            return;
        }

        if timeout.round != self.state.round() {
            return;
        }

        self.state.new_round();
        // info!("Timeout, starting new round #{}", self.state.round());
        if self.is_leader() {
            self.make_propose();
        }
        self.add_timeout();
    }

    fn add_timeout(&mut self) {
        let ms = self.state.round() * self.round_timeout;
        let time = self.state.prev_time() + Duration::milliseconds(ms as i64);
        let timeout = Timeout {
            height: self.state.height(),
            round: self.state.round(),
        };
        self.events.add_timeout(timeout, time);
    }

    fn handle(&mut self, raw: RawMessage, validated: bool) {
        // TODO: check message headers (network id, protocol version)
        if !validated {
            if !raw.verify() {
                return;
            }
        }

        match Any::from_raw(raw) {
            Any::Connect(message) => self.handle_connect(message),
            Any::Propose(message) => self.handle_propose(message),
            Any::Prevote(message) => self.handle_prevote(message),
            Any::Precommit(message) => self.handle_precommit(message),
            Any::Commit(message) => self.handle_commit(message),
        }
    }

    fn handle_connect(&mut self, message: Connect) {
        // debug!("recv connect");
        let public_key = message.raw().public_key().clone();
        let address = message.addr();
        if self.state.add_peer(public_key, address) {
            // TODO: reduce double sending of connect message
            // info!("Establish connection with {}", address);
            let message = Connect::new(self.network.address().clone(), get_time(),
                                       &self.public_key, &self.secret_key);
            self.network.send_to(&mut self.events,
                                 &address,
                                 message.raw().clone()).unwrap();
        }
    }

    fn handle_propose(&mut self, propose: Propose) {
        // debug!("recv propose");
        if propose.height() > self.state.height() + 1 {
            self.state.queue(propose.raw().clone());
            return;
        }

        if propose.height() < self.state.height() + 1 {
            if !self.byzantine {
                // info!("=== Invalid block proposed, ignore ===")
            }
            return;
        }

        if propose.prev_hash() != self.state.prev_hash() {
            return;
        }

        if propose.raw().public_key() != self.state.leader(propose.round()) {
            return;
        }

        let (hash, queue) = self.state.add_propose(propose.round(),
                                                   propose.clone());

        // debug!("send prevote");
        let prevote = Prevote::new(propose.height(),
                                   propose.round(),
                                   &hash,
                                   &self.public_key,
                                   &self.secret_key);
        self.broadcast(prevote.raw().clone());
        self.handle_prevote(prevote);

        for message in queue {
            self.handle(message, true);
        }
    }

    fn handle_prevote(&mut self, prevote: Prevote) {
        // debug!("recv prevote");
        if prevote.height() > self.state.height() + 1 {
            self.state.queue(prevote.raw().clone());
            return;
        }

        if prevote.height() < self.state.height() + 1 {
            return;
        }

        let has_consensus = self.state.add_prevote(prevote.round(),
                                                   prevote.hash(),
                                                   prevote.clone());

        if has_consensus {
            self.state.lock_round(prevote.round());
            // debug!("send precommit");
            let precommit = Precommit::new(prevote.height(),
                                           prevote.round(),
                                           prevote.hash(),
                                           &self.public_key,
                                           &self.secret_key);
            self.broadcast(precommit.raw().clone());
            self.handle_precommit(precommit);
        }
    }

    fn handle_precommit(&mut self, precommit: Precommit) {
        // debug!("recv precommit");
        if precommit.height() > self.state.height() + 1 {
            self.state.queue(precommit.raw().clone());
            return;
        }

        if precommit.height() < self.state.height() + 1 {
            return;
        }

        let has_consensus = self.state.add_precommit(precommit.round(),
                                                     precommit.hash(),
                                                     precommit.clone());

        if has_consensus {
            let queue = self.state.new_height(precommit.hash().clone());
            // info!("Commit block #{}", self.state.height());
            if self.is_leader() {
                self.make_propose();
            } else {
                // debug!("send commit");
                // let commit = Commit::new(precommit.height(),
                //                          precommit.hash(),
                //                          &self.public_key,
                //                          &self.secret_key);
                // self.broadcast(commit.clone());
                // self.handle_commit(commit);
            }
            for message in queue {
                self.handle(message, true);
            }
            self.add_timeout();
        }
    }

    fn handle_commit(&mut self, _: Commit) {
        // debug!("recv commit");
        // nothing
    }

    fn is_leader(&self) -> bool {
        self.state.leader(self.state.round()) == &self.public_key
    }

    fn make_propose(&mut self) {
        // debug!("send propose");
        // FIXME: remove this sheet
        // ::std::thread::sleep(::std::time::Duration::from_millis(self.propose_timeout as u64));
        let height = if self.byzantine {
            // info!("=== Propose invalid block ===");
            0
        } else {
            self.state.height() + 1
        };
        let propose = Propose::new(height,
                                   self.state.round(),
                                   get_time(),
                                   self.state.prev_hash(),
                                   &self.public_key,
                                   &self.secret_key);
        self.broadcast(propose.raw().clone());
        self.handle_propose(propose);
    }

    // fn send_to(&mut self, address: &net::SocketAddr, message: RawMessage) {
    //     self.network.send_to(&mut self.events, address, message).unwrap();
    // }


    // TODO: use Into<RawMessage>
    fn broadcast(&mut self, message: RawMessage) {
        for address in self.state.peers().values() {
            self.network.send_to(&mut self.events,
                                 address,
                                 message.clone()).unwrap();
        }
    }
}
