use std::net::SocketAddr;

use time::{get_time, Duration};

use super::crypto::{PublicKey, SecretKey};
use super::events::{Events, Event, Timeout, EventsConfiguration};
use super::network::{Network, NetworkConfiguration};
use super::messages::{Any, Connect, RawMessage, Message};

mod state;
mod basic;
mod consensus;

pub use self::state::{State};
pub use self::basic::{Basic, BasicService};
pub use self::consensus::{Consensus, ConsensusService};

// TODO: avoid recursion calls?

pub struct Node {
    context: NodeContext,
    basic: Box<BasicService>,
    consensus: Box<ConsensusService>,
}

pub struct NodeContext {
    pub id: u32, // TODO: validator ID, move to ConsensusService
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub state: State,
    pub events: Events,
    pub network: Network,
    pub propose_timeout: u32,
    pub round_timeout: u32,
    pub byzantine: bool,
    // TODO: move this into peer exchange service
    pub peer_discovery: Vec<SocketAddr>
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
        let id = config.validators.iter()
                                  .position(|pk| pk == &config.public_key)
                                  .unwrap();
        let events = Events::with_config(config.events).unwrap();
        let network = Network::with_config(config.network);
        let state = State::new(config.validators);
        let basic = Box::new(Basic) as Box<BasicService>;
        let consensus = Box::new(Consensus) as Box<ConsensusService>;
        Node {
            context: NodeContext {
                id: id as u32,
                public_key: config.public_key,
                secret_key: config.secret_key,
                state: state,
                events: events,
                network: network,
                propose_timeout: config.propose_timeout,
                round_timeout: config.round_timeout,
                peer_discovery: config.peer_discovery,
                byzantine: config.byzantine
            },
            basic: basic,
            consensus: consensus,
        }
    }

    fn initialize(&mut self) {
        // info!("Start listening...");
        self.context.network.bind(&mut self.context.events).unwrap();
        let message = Connect::new(&self.context.public_key,
                                   self.context.network.address().clone(),
                                   get_time(),
                                   &self.context.secret_key);
        for address in self.context.peer_discovery.iter() {
            if address == self.context.network.address() {
                continue
            }
            self.context.network.send_to(&mut self.context.events,
                                 address,
                                 message.raw().clone()).unwrap();
        }

        self.context.add_timeout();
    }

    pub fn run(&mut self) {
        self.initialize();
        loop {
            if self.context.state.height() == 1000 {
                break;
            }
            match self.context.events.poll() {
                Event::Incoming(message) => {
                    self.handle(message);
                },
                Event::Internal(_) => {

                },
                Event::Timeout(timeout) => {
                    self.handle_timeout(timeout);
                },
                Event::Io(id, set) => {
                    // TODO: shoud we call network.io through main event queue?
                    // FIXME: Remove unwrap here
                    self.context.network.io(&mut self.context.events, id, set).unwrap()
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
        if timeout.height != self.context.state.height() {
            return;
        }

        if timeout.round != self.context.state.round() {
            return;
        }

        self.context.state.new_round();
        // info!("Timeout, starting new round #{}", self.context.state.round());
        if self.consensus.is_leader(&mut self.context) {
            self.consensus.make_propose(&mut self.context);
        }
        self.context.add_timeout();
    }

    fn handle(&mut self, raw: RawMessage) {
        // TODO: check message headers (network id, protocol version)
        // FIXME: call message.verify method
        //     if !raw.verify() {
        //         return;
        //     }

        match Any::from_raw(raw).unwrap() {
            Any::Basic(message) => self.basic.handle(&mut self.context, message),
            Any::Consensus(message) => self.consensus.handle(&mut self.context, message),
            Any::Tx(_) => panic!("tx handling not implemented")
        }
    }
}

impl NodeContext {
    // fn send_to(&mut self, address: &net::SocketAddr, message: RawMessage) {
    //     self.network.send_to(&mut self.context.events, address, message).unwrap();
    // }

    pub fn add_timeout(&mut self) {
        let ms = self.state.round() * self.round_timeout;
        let time = self.state.prev_time() + Duration::milliseconds(ms as i64);
        let timeout = Timeout {
            height: self.state.height(),
            round: self.state.round(),
        };
        self.events.add_timeout(timeout, time);
    }


    // TODO: use Into<RawMessage>
    pub fn broadcast(&mut self, message: RawMessage) {
        for address in self.state.peers().values() {
            self.network.send_to(&mut self.events,
                                 address,
                                 message.clone()).unwrap();
        }
    }
}
