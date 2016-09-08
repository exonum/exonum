use std::net::SocketAddr;

use time::{Duration, Timespec};

use super::crypto::{PublicKey, SecretKey};
use super::events::{Reactor, Events, Event, NodeTimeout, EventsConfiguration, Network,
                    NetworkConfiguration, InternalEvent};
use super::blockchain::{Blockchain};
use super::messages::{Any, Connect, RawMessage};

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
    pub peers_timeout: u32,
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
    pub peers_timeout: u32,
    pub peer_discovery: Vec<SocketAddr>,
    pub validators: Vec<PublicKey>,
}

impl<B: Blockchain> Node<B> {
    pub fn new(mut blockchain: B, reactor: Box<Reactor>, config: Configuration) -> Node<B> {
        // FIXME: remove unwraps here, use FATAL log level instead
        let id = config.validators
            .iter()
            .position(|pk| pk == &config.public_key)
            .unwrap();
        let connect = Connect::new(&config.public_key,
                                   reactor.address(),
                                   reactor.get_time(),
                                   &config.secret_key);

        let last_hash = blockchain.last_hash().unwrap().unwrap_or_else(|| super::crypto::hash(&[]));

        let state = State::new(id as u32, config.validators, connect, last_hash);
        Node {
            public_key: config.public_key,
            secret_key: config.secret_key,
            state: state,
            events: reactor,
            blockchain: blockchain,
            round_timeout: config.round_timeout,
            status_timeout: config.status_timeout,
            peers_timeout: config.peers_timeout,
            peer_discovery: config.peer_discovery,
        }
    }

    pub fn with_config(blockchain: B, config: Configuration) -> Node<B> {
        // FIXME: remove unwraps here, use FATAL log level instead
        let network = Network::with_config(config.network);
        let reactor =
            Box::new(Events::with_config(config.events.clone(), network).unwrap()) as Box<Reactor>;
        Self::new(blockchain, reactor, config)
    }

    pub fn state(&self) -> &State<B::Transaction> {
        &self.state
    }

    pub fn initialize(&mut self) {
        info!("Start listening...");
        self.events.bind().unwrap();

        for address in &self.peer_discovery.clone() {
            if address == &self.events.address() {
                continue;
            }
            self.connect(address);
            info!("Try to connect with peer {}", address);
        }

        // TODO: rewrite this bullshit
        let time = self.blockchain
            .last_block()
            .unwrap()
            .map_or_else(|| Timespec { sec: 0, nsec: 0 }, |p| p.time());
        let round = 1 +
                    (self.events.get_time() - time).num_milliseconds() / self.round_timeout as i64;
        self.state.jump_round(round as u32);

        info!("Jump to round {}", round);

        self.add_round_timeout();
        self.add_status_timeout();
        self.add_peer_exchange_timeout();
    }

    pub fn run(&mut self) {
        self.initialize();
        loop {
            if self.state.height() == 1000 {
                break;
            }

            let event = self.events.poll();
            match event {
                Event::Terminate => break,
                _ => self.handle_event(event),
            }
        }
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Incoming(message) => {
                self.handle_message(message);
            }
            Event::Timeout(timeout) => {
                self.handle_timeout(timeout);
            }
            Event::Internal(internal) => {
                match internal {
                    InternalEvent::Error(_) => {}
                    InternalEvent::Connected(addr) => self.handle_connected(&addr),
                    InternalEvent::Disconnected(addr) => self.handle_disconnected(&addr),
                }
            }
            Event::Terminate => {}
        }
    }

    pub fn handle_message(&mut self, raw: RawMessage) {
        // TODO: check message headers (network id, protocol version)
        // FIXME: call message.verify method
        //     if !raw.verify() {
        //         return;
        //     }
        let msg = Any::from_raw(raw).unwrap();
        match msg {
            Any::Connect(msg) => self.handle_connect(msg),
            Any::Status(msg) => self.handle_status(msg),
            Any::Transaction(message) => self.handle_tx(message),
            Any::Consensus(message) => self.handle_consensus(message),
            Any::Request(message) => self.handle_request(message),
        }
    }

    pub fn handle_timeout(&mut self, timeout: NodeTimeout) {
        match timeout {
            NodeTimeout::Round(height, round) => self.handle_round_timeout(height, round),
            NodeTimeout::Request(data, peer) => self.handle_request_timeout(data, peer),
            NodeTimeout::Status => self.handle_status_timeout(),
            NodeTimeout::PeerExchange => self.handle_peer_exchange_timeout(),
        }
    }

    pub fn send_to_validator(&mut self, id: u32, message: &RawMessage) {
        // TODO: check validator id
        let public_key = self.state.validators()[id as usize];
        self.send_to_peer(public_key, message);
    }

    pub fn send_to_peer(&mut self, public_key: PublicKey, message: &RawMessage) {
        if let Some(conn) = self.state.peers().get(&public_key) {
            self.events.send_to(&conn.addr(), message.clone());
        } else {
            warn!("Hasn't connection with peer {:?}", public_key);
        }
    }

    pub fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        self.events.send_to(address, message.clone());
    }

    pub fn connect(&mut self, address: &SocketAddr) {
        self.events.connect(address);
    }

    pub fn request(&mut self, data: RequestData, peer: PublicKey) {
        let is_new = self.state.request(data.clone(), peer);

        if is_new {
            debug!("ADD REQUEST TIMEOUT");
            self.add_request_timeout(data, None);
        }
    }

    pub fn add_round_timeout(&mut self) {
        let ms = self.state.round() as i64 * self.round_timeout as i64;
        let time = self.blockchain
            .last_block()
            .unwrap()
            .map_or_else(|| Timespec { sec: 0, nsec: 0 }, |p| p.time()) +
                   Duration::milliseconds(ms);
        debug!("ADD ROUND TIMEOUT, time={:?}, height={}, round={}",
              time,
              self.state.height(),
              self.state.round());
        let timeout = NodeTimeout::Round(self.state.height(), self.state.round());
        self.events.add_timeout(timeout, time);
    }

    pub fn add_status_timeout(&mut self) {
        let time = self.events.get_time() + Duration::milliseconds(self.status_timeout as i64);
        self.events.add_timeout(NodeTimeout::Status, time);
    }

    pub fn add_request_timeout(&mut self, data: RequestData, peer: Option<PublicKey>) {
        let time = self.events.get_time() + data.timeout();
        self.events.add_timeout(NodeTimeout::Request(data, peer), time);
    }

    pub fn add_peer_exchange_timeout(&mut self) {
        let time = self.events.get_time() + Duration::milliseconds(self.peers_timeout as i64);
        self.events.add_timeout(NodeTimeout::PeerExchange, time);
    }

    // TODO: use Into<RawMessage>
    pub fn broadcast(&mut self, message: &RawMessage) {
        for conn in self.state.peers().values() {
            self.events.send_to(&conn.addr(), message.clone());
        }
    }
}
