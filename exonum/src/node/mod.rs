use std::io;
use std::net::SocketAddr;
use std::marker::PhantomData;

use time::{Duration, Timespec};

use super::crypto::{PublicKey, SecretKey, Hash, hash};
use super::events::{Events, MioChannel, EventLoop, Reactor, Network, NetworkConfiguration, Event,
                    EventsConfiguration, Channel, EventHandler, Result as EventsResult,
                    Error as EventsError};
use super::blockchain::Blockchain;

use super::messages::{Connect, RawMessage};

pub mod state;//temporary solution to get access to WAIT consts
mod basic;
mod consensus;
mod requests;
mod configuration;
mod adjusted_propose_timeout;
pub mod config;

use super::config::view::{StoredConfiguration, ConsensusCfg};
pub use self::config::ListenerConfig;
pub use self::state::{State, Round, Height, RequestData, ValidatorId};
use self::adjusted_propose_timeout::*;

type ProposeTimeoutAdjusterType = adjusted_propose_timeout::MovingAverageProposeTimeoutAdjuster;

pub const GENESIS_TIME: Timespec = Timespec {
    sec: 1451649600,
    nsec: 0,
};

#[derive(Clone, Debug)]
pub enum ExternalMessage<B: Blockchain> {
    Transaction(B::Transaction),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum NodeTimeout {
    Status,
    Round(u64, u32),
    Request(RequestData, Option<PublicKey>),
    Propose(u64, u32),
    PeerExchange,
}

#[derive(Clone)]
pub struct TxSender<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout>
{
    inner: S,
    _b: PhantomData<B>,
}

pub struct NodeHandler<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout>
{
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub state: State,
    pub channel: S,
    pub blockchain: B,
    // TODO: move this into peer exchange service
    pub peer_discovery: Vec<SocketAddr>,
    propose_timeout_adjuster: Box<adjusted_propose_timeout::ProposeTimeoutAdjuster<B>>,
}

#[derive(Debug, Clone)]
pub struct Configuration {
    pub listener: ListenerConfig,
    pub events: EventsConfiguration,
    pub network: NetworkConfiguration,
    pub consensus: ConsensusCfg,
    pub peer_discovery: Vec<SocketAddr>,
    pub validators: Vec<PublicKey>,
}

impl Configuration {
    pub fn update_with_actual_config(&mut self, actual_config: StoredConfiguration){
        self.validators = actual_config.validators;
        self.consensus = actual_config.consensus;
    }
}

impl<B, S> NodeHandler<B, S>
    where B: Blockchain,
          S: Channel<ApplicationEvent = ExternalMessage<B>, Timeout = NodeTimeout> + Clone
{
    pub fn new(mut blockchain: B, sender: S, mut config: Configuration) -> NodeHandler<B, S> {
        // FIXME: remove unwraps here, use FATAL log level instead

        let r = blockchain.last_block().unwrap();
        // TODO нужно создать api и для того, чтобы здесь подключался genesis блок
        let (last_hash, last_height) = if let Some(last_block) = r {
            (last_block.hash(), last_block.height() + 1)
        } else {
            (super::crypto::hash(&[]), 0)
        };

        let id = config.validators
            .iter()
            .position(|pk| pk == &config.listener.public_key)
            .unwrap();

        let connect = Connect::new(&config.listener.public_key,
                                   sender.address(),
                                   sender.get_time(),
                                   &config.listener.secret_key);

        if let Some(stored_config) = blockchain.get_initial_configuration() {
            config.update_with_actual_config(stored_config);
        }

        let state = State::new(
            id as u32,
            config.validators,
            connect,
            last_hash,
            last_height,
            ConsensusCfg{
                round_timeout: config.consensus.round_timeout as i64,
                propose_timeout: config.consensus.propose_timeout as i64,
                status_timeout: config.consensus.status_timeout as i64,
                peers_timeout: config.consensus.peers_timeout as i64,
                txs_block_limit: config.consensus.txs_block_limit,
            }
        );

        NodeHandler {
            public_key: config.listener.public_key,
            secret_key: config.listener.secret_key,
            state: state,
            channel: sender,
            blockchain: blockchain,
//            propose_timeout_adjuster:   Box::new(adjusted_propose_timeout::MovingAverageProposeTimeoutAdjuster::default()),
            propose_timeout_adjuster:   Box::new(adjusted_propose_timeout::ConstProposeTimeout{ propose_timeout: config.consensus.propose_timeout as i64, }),
            peer_discovery: config.peer_discovery
        }
    }

    pub fn propose_timeout(&self) -> i64 {
        self.state().consensus_config().propose_timeout
    }

    pub fn round_timeout(&self) -> i64 {
        self.state().consensus_config().round_timeout
    }

    pub fn status_timeout(&self) -> i64 {
        self.state().consensus_config().status_timeout
    }

    pub fn peers_timeout(&self) -> i64 {
        self.state().consensus_config().peers_timeout
    }

    pub fn txs_block_limit(&self) -> u32 {
        self.state().consensus_config().txs_block_limit
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn initialize(&mut self) {
        info!("Start listening address={}", self.channel.address());
        for address in &self.peer_discovery.clone() {
            if address == &self.channel.address() {
                continue;
            }
            self.connect(address);
            info!("Try to connect with peer {}", address);
        }

        let round = self.actual_round();
        self.state.jump_round(round);

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
            trace!("Send to addr: {}", conn.addr());
            self.channel.send_to(&conn.addr(), message.clone());
        } else {
            warn!("Hasn't connection with peer {:?}", public_key);
        }
    }

    pub fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        trace!("Send to addr: {}", address);
        self.channel.send_to(address, message.clone());
    }

    // TODO: use Into<RawMessage>
    pub fn broadcast(&mut self, message: &RawMessage) {
        for conn in self.state.peers().values() {
            trace!("Send to addr: {}", conn.addr());
            self.channel.send_to(&conn.addr(), message.clone());
        }
    }

    pub fn connect(&mut self, address: &SocketAddr) {
        self.channel.connect(address);
    }

    pub fn request(&mut self, data: RequestData, peer: PublicKey) {
        let is_new = self.state.request(data.clone(), peer);
        if is_new {
            self.add_request_timeout(data, None);
        }
    }

    pub fn add_round_timeout(&mut self) {
        let time = self.round_start_time(self.state.round() + 1);
        trace!("ADD ROUND TIMEOUT, time={:?}, height={}, round={}, elapsed={}ms",
               time,
               self.state.height(),
               self.state.round(),
               (time - self.channel.get_time()).num_milliseconds());
        let timeout = NodeTimeout::Round(self.state.height(), self.state.round());
        self.channel.add_timeout(timeout, time);
    }

    ///getter for adjusted_propose_timeout
    pub fn adjusted_propose_timeout(&self) -> i64 {
        self.propose_timeout_adjuster.adjusted_propose_timeout(&self.blockchain.view())
    }

    pub fn add_propose_timeout(&mut self) {
//        let time = self.round_start_time(self.state.round()) +
//                   Duration::milliseconds(self.propose_timeout);
        let adjusted_propose_timeout = self.adjusted_propose_timeout();//cache adjusted_propose_timeout because this value will be used 2 times
        let time = self.round_start_time(self.state.round()) +
                   Duration::milliseconds(adjusted_propose_timeout);
        self.propose_timeout_adjuster.update_last_propose_timeout(adjusted_propose_timeout);

        trace!("ADD PROPOSE TIMEOUT, time={:?}, height={}, round={}, elapsed={}ms",
               time,
               self.state.height(),
               self.state.round(),
               (time - self.channel.get_time()).num_milliseconds());
        let timeout = NodeTimeout::Propose(self.state.height(), self.state.round());
        self.channel.add_timeout(timeout, time);
    }

    pub fn add_status_timeout(&mut self) {
        let time = self.channel.get_time() + Duration::milliseconds(self.status_timeout());
        self.channel.add_timeout(NodeTimeout::Status, time);
    }

    pub fn add_request_timeout(&mut self, data: RequestData, peer: Option<PublicKey>) {
        trace!("ADD REQUEST TIMEOUT");
        let time = self.channel.get_time() + data.timeout();
        self.channel.add_timeout(NodeTimeout::Request(data, peer), time);
    }

    pub fn add_peer_exchange_timeout(&mut self) {
        let time = self.channel.get_time() + Duration::milliseconds(self.peers_timeout());
        self.channel.add_timeout(NodeTimeout::PeerExchange, time);
    }

    pub fn last_block_time(&self) -> Timespec {
        self.blockchain
            .last_block()
            .unwrap()
            .map_or_else(|| GENESIS_TIME, |p| p.time())
    }

    pub fn last_block_hash(&self) -> Hash {
        self.blockchain
            .last_block()
            .unwrap()
            .map_or_else(|| hash(&[]), |p| p.hash())
    }

    pub fn actual_round(&self) -> Round {
        let now = self.channel.get_time();
        let propose = self.last_block_time();
        debug_assert!(now >= propose);

        let duration = (now - propose - Duration::milliseconds(self.adjusted_propose_timeout()))
            .num_milliseconds();
        if duration > 0 {
            let round = (duration / self.round_timeout()) as Round + 1;
            ::std::cmp::max(1, round)
        } else {
            1
        }
    }

    // FIXME find more flexible solution
    pub fn round_start_time(&self, round: Round) -> Timespec {
        let ms = (round - 1) as i64 * self.round_timeout();
        self.last_block_time() + Duration::milliseconds(ms)
    }
}

impl<B, S> EventHandler for NodeHandler<B, S>
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
            NodeTimeout::Propose(height, round) => self.handle_propose_timeout(height, round),
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

    pub fn send(&self, tx: B::Transaction) -> EventsResult<()> {
        if B::verify_tx(&tx) {
            let msg = ExternalMessage::Transaction(tx);
            self.inner.post_event(msg)?;
            Ok(())
        } else {
            Err(EventsError::new("Unable to verify transacion"))
        }
    }
}

pub type NodeChannel<B> = MioChannel<ExternalMessage<B>, NodeTimeout>;

pub struct Node<B>
    where B: Blockchain
{
    reactor: Events<NodeHandler<B, NodeChannel<B>>>,
}

impl<B> Node<B>
    where B: Blockchain
{
    pub fn new(blockchain: B, config: Configuration) -> Node<B> {
        let network = Network::with_config(config.listener.address, config.network);
        let event_loop = EventLoop::configured(config.events.clone()).unwrap();
        let channel = MioChannel::new(config.listener.address, event_loop.channel());
        let worker = NodeHandler::new(blockchain, channel, config);
        Node { reactor: Events::with_event_loop(network, worker, event_loop) }
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.reactor.bind()?;
        self.reactor.handler_mut().initialize();
        self.reactor.run()
    }

    pub fn state(&self) -> &State {
        self.reactor.handler().state()
    }

    pub fn channel(&self) -> TxSender<B, NodeChannel<B>> {
        TxSender::new(self.reactor.handler().channel.clone())
    }
}
