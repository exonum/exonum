use std::net::SocketAddr;
use std::marker::PhantomData;

use time::{Duration, Timespec};

use super::crypto::{PublicKey, SecretKey};
use super::events::{Event, EventsConfiguration, NetworkConfiguration,
                    Channel, EventHandler};
use super::blockchain::Blockchain;
use super::messages::{Connect, RawMessage};

mod state;
mod basic;
mod consensus;
mod requests;

pub use self::state::{State, Round, Height, RequestData, ValidatorId};

#[derive(Debug)]
pub enum ExternalMessage<B: Blockchain> {
    Transaction(B::Transaction),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum NodeTimeout {
    Status,
    Round(u64, u32),
    Request(RequestData, Option<PublicKey>),
    PeerExchange,
}

pub struct TxSender<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout>
{
    inner: S,
    _b: PhantomData<B>,
}

// TODO: avoid recursion calls?

pub struct Node<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout>
{
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub state: State<B::Transaction>,
    pub channel: S,
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

impl<B, S> Node<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout> + Clone
{
    pub fn new(blockchain: B, sender: S, config: Configuration) -> Node<B, S> {
        // FIXME: remove unwraps here, use FATAL log level instead
        let id = config.validators
            .iter()
            .position(|pk| pk == &config.public_key)
            .unwrap();
        let connect = Connect::new(&config.public_key,
                                   sender.address(),
                                   sender.get_time(),
                                   &config.secret_key);

        let last_hash = blockchain.last_hash().unwrap().unwrap_or_else(|| super::crypto::hash(&[]));

        let state = State::new(id as u32, config.validators, connect, last_hash);
        Node {
            public_key: config.public_key,
            secret_key: config.secret_key,
            state: state,
            channel: sender,
            blockchain: blockchain,
            round_timeout: config.round_timeout,
            status_timeout: config.status_timeout,
            peers_timeout: config.peers_timeout,
            peer_discovery: config.peer_discovery,
        }
    }

    pub fn state(&self) -> &State<B::Transaction> {
        &self.state
    }

    pub fn initialize(&mut self) {
        info!("Start listening...");
        for address in &self.peer_discovery.clone() {
            if address == &self.channel.address() {
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
                    (self.channel.get_time() - time).num_milliseconds() / self.round_timeout as i64;
        self.state.jump_round(round as u32);

        info!("Jump to round {}", round);

        self.add_round_timeout();
        self.add_status_timeout();
        self.add_peer_exchange_timeout();
    }

    pub fn send_to_validator(&mut self, id: u32, message: &RawMessage) {
        // TODO: check validator id
        let public_key = self.state.validators()[id as usize];
        self.send_to_peer(public_key, message);
    }

    pub fn send_to_peer(&mut self, public_key: PublicKey, message: &RawMessage) {
        if let Some(conn) = self.state.peers().get(&public_key) {
            self.channel.send_to(&conn.addr(), message.clone());
        } else {
            warn!("Hasn't connection with peer {:?}", public_key);
        }
    }

    pub fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        self.channel.send_to(address, message.clone());
    }

    pub fn connect(&mut self, address: &SocketAddr) {
        self.channel.connect(address);
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
        self.channel.add_timeout(timeout, time);
    }

    pub fn add_status_timeout(&mut self) {
        let time = self.channel.get_time() + Duration::milliseconds(self.status_timeout as i64);
        self.channel.add_timeout(NodeTimeout::Status, time);
    }

    pub fn add_request_timeout(&mut self, data: RequestData, peer: Option<PublicKey>) {
        let time = self.channel.get_time() + data.timeout();
        self.channel.add_timeout(NodeTimeout::Request(data, peer), time);
    }

    pub fn add_peer_exchange_timeout(&mut self) {
        let time = self.channel.get_time() + Duration::milliseconds(self.peers_timeout as i64);
        self.channel.add_timeout(NodeTimeout::PeerExchange, time);
    }

    // TODO: use Into<RawMessage>
    pub fn broadcast(&mut self, message: &RawMessage) {
        for conn in self.state.peers().values() {
            self.channel.send_to(&conn.addr(), message.clone());
        }
    }

    pub fn channel(&self) -> TxSender<B, S> {
        TxSender::new(self.channel.clone())
    }
}

impl<B, S> EventHandler for Node<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout> + Clone
{
    type Timeout = NodeTimeout;
    type ApplicationEvent = ExternalMessage<B>;

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Connected(addr) => self.handle_connected(&addr),
            Event::Disconnected(addr) => self.handle_disconnected(&addr),
            Event::Incoming(raw) => self.handle_message(raw),
            Event::Error(_) => {}
        }
    }

    fn handle_application_event(&mut self, event: Self::ApplicationEvent) {
        match event {
            ExternalMessage::Transaction(tx) => {
                self.handle_incoming_tx(tx);
            }
        }
    }

    fn handle_timeout(&mut self, timeout: Self::Timeout) {
        match timeout {
            NodeTimeout::Round(height, round) => self.handle_round_timeout(height, round),
            NodeTimeout::Request(data, peer) => self.handle_request_timeout(data, peer),
            NodeTimeout::Status => self.handle_status_timeout(),
            NodeTimeout::PeerExchange => self.handle_peer_exchange_timeout(),
        }
    }
}

impl<B, S> TxSender<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout>
{
    pub fn new(inner: S) -> TxSender<B, S> {
        TxSender {
            inner: inner,
            _b: PhantomData,
        }
    }

    // TODO handle error
    pub fn send(&self, tx: B::Transaction) {
        if B::verify_tx(&tx) {
            let msg = ExternalMessage::Transaction(tx);
            self.inner.post_event(msg).unwrap();
        }
    }
}
