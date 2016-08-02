use std::net::SocketAddr;

use time::{Duration, Timespec};

use super::crypto::{PublicKey, SecretKey};
use super::events::{Reactor, Events, Event, Timeout, EventsConfiguration};
use super::network::{Network, NetworkConfiguration, PeerId, EventSet};
use super::storage::{Storage, MemoryDB};
use super::messages::{Any, Connect, RawMessage, Message};
use super::tx_generator::TxGenerator;

mod state;
mod basic;
mod consensus;
mod requests;

pub use self::state::{State, Round, Height, RequestData, ValidatorId};
pub use self::basic::{BasicService, BasicHandler};
pub use self::consensus::{ConsensusService, ConsensusHandler};
pub use self::requests::{RequestService, RequestHandler};

// TODO: avoid recursion calls?

pub struct Node {
    context: NodeContext,
    basic: Box<BasicHandler>,
    consensus: Box<ConsensusHandler>,
    requests: Box<RequestHandler>,
}

pub struct NodeContext {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub state: State,
    pub events: Box<Reactor>,
    pub storage: Storage<MemoryDB>,
    pub propose_timeout: u32,
    pub round_timeout: u32,
    pub byzantine: bool,
    // TODO: move this into peer exchange service
    pub peer_discovery: Vec<SocketAddr>,
    pub tx_generator: TxGenerator,
}

#[derive(Debug, Clone)]
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
        let id = config.validators.iter()
                                  .position(|pk| pk == &config.public_key)
                                  .unwrap();
        let tx_generator = TxGenerator::new();
        let state = State::new(id as u32, config.validators);
        let storage = Storage::new(MemoryDB::new());
        let network = Network::with_config(config.network);
        let reactor = Box::new(Events::with_config(config.events, network).unwrap()) as Box<Reactor>;
        let context = NodeContext {
            public_key: config.public_key,
            secret_key: config.secret_key,
            state: state,
            events: reactor,
            storage: storage,
            propose_timeout: config.propose_timeout,
            round_timeout: config.round_timeout,
            peer_discovery: config.peer_discovery,
            byzantine: config.byzantine,
            tx_generator: tx_generator,
        };
        Self::with_context(context)
    }

    pub fn with_context(context: NodeContext) -> Node {
        let basic = Box::new(BasicService) as Box<BasicHandler>;
        let consensus = Box::new(ConsensusService) as Box<ConsensusHandler>;
        let requests = Box::new(RequestService) as Box<RequestHandler>;
        Node {
            context: context,
            basic: basic,
            consensus: consensus,
            requests: requests,
        }
    }

    pub fn initialize(&mut self) {
        info!("Start listening...");
        self.context.events.bind().unwrap();
        let message = Connect::new(&self.context.public_key,
                                   self.context.events.address().clone(),
                                   self.context.events.get_time(),
                                   &self.context.secret_key);
        for address in self.context.peer_discovery.clone().iter() {
            if address == &self.context.events.address() {
                continue
            }
            self.context.send_to_addr(address, message.raw());
        }

        self.context.add_round_timeout();
    }

    pub fn run(&mut self) {
        self.initialize();
        loop {
            if self.context.state.height() == 1000 {
                break;
            }
            match self.context.events.poll() {
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
                    self.context.events.io(id, set).unwrap()
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
            Any::Basic(message) => self.basic.handle(&mut self.context, message),
            Any::Tx(message) => self.consensus.handle_tx(&mut self.context, message),
            Any::Consensus(message) => self.consensus.handle(&mut self.context, message),
        }
    }

    pub fn handle_timeout(&mut self, timeout: Timeout) {
        match timeout {
            Timeout::Round(height, round) =>
                self.consensus.handle_round_timeout(&mut self.context,
                                                    height, round),
            Timeout::Request(data, validator) =>
                self.consensus.handle_request_timeout(&mut self.context,
                                                      data, validator),
        }
    }
}

impl NodeContext {
    fn send_to_validator(&mut self, id: u32, message: &RawMessage) {
        // TODO: check validator id
        let public_key = self.state.validators()[id as usize];
        self.send_to_peer(public_key, message);
    }

    fn send_to_peer(&mut self, public_key: PublicKey, message: &RawMessage) {
        let &mut NodeContext {ref state, ref mut events, ..} = self;
        if let Some(addr) = state.peers().get(&public_key) {
            events.send_to(addr, message.clone()).unwrap();
        } else {
            // TODO: warning - hasn't connection with peer
        }
    }

    fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        self.events.send_to(address, message.clone()).unwrap();
    }

    fn add_round_timeout(&mut self) {
        let ms = self.state.round() * self.round_timeout;
        let time = self.storage.last_propose().unwrap().map(|p| p.time()).unwrap_or_else(|| Timespec {sec: 1469002618, nsec: 0}) + Duration::milliseconds(ms as i64);
        info!("ADD ROUND TIMEOUT, time={:?}, height={}, round={}", time, self.state.height(), self.state.round());
        let timeout = Timeout::Round(self.state.height(), self.state.round());
        self.events.add_timeout(timeout, time);
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
