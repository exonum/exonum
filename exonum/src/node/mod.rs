use std::net::SocketAddr;

use time::{Duration, Timespec};

use super::crypto::{PublicKey, SecretKey};
use super::events::{
    Reactor, Events, Event, Timeout, EventsConfiguration,
    Network, NetworkConfiguration
};
use super::storage::{Blockchain, Storage};
use super::messages::{Any, Connect, RawMessage, Message};

mod state;
mod basic;
mod consensus;
mod requests;

pub use self::state::{State, Round, Height, RequestData, ValidatorId};

// TODO: avoid recursion calls?

pub struct Node<B: Blockchain> {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub state: State<B::Transaction>,
    pub events: Box<Reactor>,
    pub blockchain: B,
    pub round_timeout: u32,
    pub status_timeout: u32,
    // TODO: move this into peer exchange service
    pub peer_discovery: Vec<SocketAddr>,
}

#[derive(Debug, Clone)]
pub struct Configuration {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub events: EventsConfiguration,
    pub network: NetworkConfiguration,
    pub round_timeout: u32,
    pub status_timeout: u32,
    pub peer_discovery: Vec<SocketAddr>,
    pub validators: Vec<PublicKey>,
}

impl<B: Blockchain> Node<B> {
    pub fn with_config(blockchain: B, config: Configuration) -> Node<B> {
        // FIXME: remove unwraps here, use FATAL log level instead
        let id = config.validators.iter()
                                  .position(|pk| pk == &config.public_key)
                                  .unwrap();
        let state = State::new(id as u32, config.validators);
        let network = Network::with_config(config.network);
        let reactor = Box::new(Events::with_config(config.events, network).unwrap()) as Box<Reactor>;
        Node {
            public_key: config.public_key,
            secret_key: config.secret_key,
            state: state,
            events: reactor,
            blockchain: blockchain,
            round_timeout: config.round_timeout,
            status_timeout: config.status_timeout,
            peer_discovery: config.peer_discovery,
        }
    }

    pub fn initialize(&mut self) {
        info!("Start listening...");
        self.events.bind().unwrap();
        let message = Connect::new(&self.public_key,
                                   self.events.address().clone(),
                                   self.events.get_time(),
                                   &self.secret_key);
        for address in self.peer_discovery.clone().iter() {
            if address == &self.events.address() {
                continue
            }
            self.send_to_addr(address, message.raw());
        }

        self.add_round_timeout();
        self.add_status_timeout();
    }

    pub fn run(&mut self) {
        self.initialize();
        loop {
            if self.state.height() == 1000 {
                break;
            }
            match self.events.poll() {
                Event::Incoming(message) => {
                    self.handle_message(message);
                },
                Event::Internal(_) => {

                },
                Event::Timeout(timeout) => {
                    self.handle_timeout(timeout);
                },
                Event::Io(id, set) => {
                    // TODO: shoud we call network.io through main event queue?
                    // FIXME: Remove unwrap here
                    self.events.io(id, set).unwrap()
                },
                Event::Error(_) => {

                },
                Event::Terminate => {
                    break
                }
            }
        }
    }

    pub fn handle_message(&mut self, raw: RawMessage) {
        // TODO: check message headers (network id, protocol version)
        // FIXME: call message.verify method
        //     if !raw.verify() {
        //         return;
        //     }

        match Any::from_raw(raw).unwrap() {
            Any::Connect(msg) => self.handle_connect(msg),
            Any::Status(msg) => self.handle_status(msg),
            Any::Transaction(message) => self.handle_tx(message),
            Any::Consensus(message) => self.handle_consensus(message),
            Any::Request(message) => self.handle_request(message),
        }
    }

    pub fn handle_timeout(&mut self, timeout: Timeout) {
        match timeout {
            Timeout::Round(height, round) =>
                self.handle_round_timeout(height, round),
            Timeout::Request(data, validator) =>
                self.handle_request_timeout(data, validator),
            Timeout::Status =>
                self.handle_status_timeout(),

        }
    }

    pub fn send_to_validator(&mut self, id: u32, message: &RawMessage) {
        // TODO: check validator id
        let public_key = self.state.validators()[id as usize];
        self.send_to_peer(public_key, message);
    }

    pub fn send_to_peer(&mut self, public_key: PublicKey, message: &RawMessage) {
        if let Some(addr) = self.state.peers().get(&public_key) {
            self.events.send_to(addr, message.clone()).unwrap();
        } else {
            // TODO: warning - hasn't connection with peer
        }
    }

    pub fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        self.events.send_to(address, message.clone()).unwrap();
    }

    pub fn request(&mut self, data: RequestData, validator: ValidatorId) {
        let is_new = self.state.request(data.clone(), validator);

        if is_new {
            self.add_request_timeout(data, validator);
        }
    }

    pub fn add_round_timeout(&mut self) {
        let ms = self.state.round() * self.round_timeout;
        let time = self.blockchain.last_propose().unwrap().map(|p| p.time()).unwrap_or_else(|| Timespec {sec: 0, nsec: 0}) + Duration::milliseconds(ms as i64);
        info!("ADD ROUND TIMEOUT, time={:?}, height={}, round={}", time, self.state.height(), self.state.round());
        let timeout = Timeout::Round(self.state.height(), self.state.round());
        self.events.add_timeout(timeout, time);
    }

    pub fn add_status_timeout(&mut self) {
        let time = self.events.get_time() + Duration::milliseconds(self.status_timeout as i64);
        self.events.add_timeout(Timeout::Status, time);
    }

    pub fn add_request_timeout(&mut self,
                               data: RequestData,
                               validator: ValidatorId) {
        let time = self.events.get_time() + data.timeout();
        self.events.add_timeout(Timeout::Request(data, validator), time);
    }

    // TODO: use Into<RawMessage>
    pub fn broadcast(&mut self, message: &RawMessage) {
        for address in self.state.peers().values() {
            self.events.send_to(address, message.clone()).unwrap();
        }
    }
}
