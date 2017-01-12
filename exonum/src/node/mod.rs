use std::io;
use std::net::SocketAddr;

use time::{Duration, Timespec};

use super::crypto::{PublicKey, SecretKey, Hash};
use super::events::{Events, MioChannel, EventLoop, Reactor, Network, NetworkConfiguration, Event,
                    EventsConfiguration, Channel, EventHandler, Result as EventsResult,
                    Error as EventsError};
use super::blockchain::{Blockchain, Schema, GenesisConfig, Transaction};
use super::messages::{Connect, RawMessage};

pub mod state;//temporary solution to get access to WAIT consts

mod basic;
mod consensus;
mod requests;
mod adjusted_propose_timeout;

pub use self::state::{State, Round, Height, RequestData, ValidatorId};
use self::adjusted_propose_timeout::*;

pub type ProposeTimeoutAdjusterType = adjusted_propose_timeout::MovingAverageProposeTimeoutAdjuster;

#[derive(Clone, Debug)]
pub enum ExternalMessage {
    Transaction(Box<Transaction>),
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
pub struct TxSender<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    inner: S,
}

pub struct NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub state: State,
    pub channel: S,
    pub blockchain: Blockchain,
    // TODO: move this into peer exchange service
    pub peer_discovery: Vec<SocketAddr>,
    propose_timeout_adjuster: Box<adjusted_propose_timeout::ProposeTimeoutAdjuster>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListenerConfig {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub address: SocketAddr,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub genesis: GenesisConfig,
    pub listen_address: SocketAddr,
    pub network: NetworkConfiguration,
    pub peers: Vec<SocketAddr>,
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
}

#[derive(Debug, Clone)]
pub struct Configuration {
    pub listener: ListenerConfig,
    pub events: EventsConfiguration,
    pub network: NetworkConfiguration,
    pub peer_discovery: Vec<SocketAddr>,
}

impl<S> NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout> + Clone
{
    pub fn new(blockchain: Blockchain, sender: S, config: Configuration) -> NodeHandler<S> {
        // FIXME: remove unwraps here, use FATAL log level instead
        let (last_hash, last_height) = {
            let block = blockchain.last_block().unwrap();
            (block.hash(), block.height() + 1)
        };

        let stored = Schema::new(&blockchain.view()).get_actual_configuration().unwrap();
        info!("Create node with config={:#?}", stored);

        let id = stored.validators
            .iter()
            .position(|pk| pk == &config.listener.public_key)
            .unwrap();

        let connect = Connect::new(&config.listener.public_key,
                                   sender.address(),
                                   sender.get_time(),
                                   &config.listener.secret_key);

        let propose_timeout_adjuster = Box::new(adjusted_propose_timeout::ConstProposeTimeout {
            propose_timeout: stored.consensus.propose_timeout as i64,
        });

        let state = State::new(id as u32,
                               stored.validators,
                               connect,
                               last_hash,
                               last_height,
                               stored.consensus);

        NodeHandler {
            public_key: config.listener.public_key,
            secret_key: config.listener.secret_key,
            state: state,
            channel: sender,
            blockchain: blockchain,
            propose_timeout_adjuster: propose_timeout_adjuster,
            peer_discovery: config.peer_discovery,
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

    /// getter for adjusted_propose_timeout
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
            .time()
    }

    pub fn last_block_hash(&self) -> Hash {
        self.blockchain
            .last_block()
            .unwrap()
            .hash()
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

impl<S> EventHandler for NodeHandler<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout> + Clone
{
    type Timeout = NodeTimeout;
    type ApplicationEvent = ExternalMessage;

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

impl<S> TxSender<S>
    where S: Channel<ApplicationEvent = ExternalMessage, Timeout = NodeTimeout>
{
    pub fn new(inner: S) -> TxSender<S> {
        TxSender { inner: inner }
    }

    pub fn send<T: Transaction>(&self, tx: T) -> EventsResult<()> {
        // TODO remove double data convertation
        if !tx.verify() {
            return Err(EventsError::new("Unable to verify transaction"));
        }
        let msg = ExternalMessage::Transaction(Box::new(tx));
        self.inner.post_event(msg)
    }
}

pub type NodeChannel = MioChannel<ExternalMessage, NodeTimeout>;

pub struct Node {
    reactor: Events<NodeHandler<NodeChannel>>,
}

impl Node {
    pub fn new(blockchain: Blockchain, node_cfg: NodeConfig) -> Node {
        blockchain.create_genesis_block(node_cfg.genesis.clone()).unwrap();

        let config = Configuration {
            listener: ListenerConfig {
                public_key: node_cfg.public_key,
                secret_key: node_cfg.secret_key,
                address: node_cfg.listen_address,
            },
            network: node_cfg.network,
            events: EventsConfiguration::default(),
            peer_discovery: node_cfg.peers,
        };
        let network = Network::with_config(node_cfg.listen_address, config.network);
        let event_loop = EventLoop::configured(config.events.clone()).unwrap();
        let channel = MioChannel::new(node_cfg.listen_address, event_loop.channel());
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

    pub fn channel(&self) -> TxSender<NodeChannel> {
        TxSender::new(self.reactor.handler().channel.clone())
    }
}
