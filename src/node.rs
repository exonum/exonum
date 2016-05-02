use std::{net};
use time::{get_time};

use super::crypto::{PublicKey, SecretKey};
use super::events::{Events, Event, EventsConfiguration};
use super::network::{Network, NetworkConfiguration};
use super::message::{Message, RawMessage, ProtocolMessage};
use super::protocol::{Connect, Propose, Prevote, Precommit, Commit};
use super::state::{State};

pub struct Node {
    public_key: PublicKey,
    secret_key: SecretKey,
    state: State,
    events: Events,
    network: Network,
    // TODO: move this into peer exchange service
    peer_discovery: Vec<net::SocketAddr>
}

#[derive(Debug)]
pub struct Configuration {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub events: EventsConfiguration,
    pub network: NetworkConfiguration,
    pub peer_discovery: Vec<net::SocketAddr>
}

impl Node {
    pub fn with_config(config: Configuration) -> Node {
        // FIXME: remove unwraps here, use FATAL log level instead
        let events = Events::with_config(config.events).unwrap();
        let network = Network::with_config(config.network);
        let mut state = State::new();
        state.add_validator(config.public_key, config.network.listen_address);
        Node {
            public_key: config.public_key,
            secret_key: config.secret_key,
            state: state,
            events: events,
            network: network,
            peer_discovery: config.peer_discovery
        }
    }

    fn initialize(&mut self) {
        info!("initialize");
        self.network.bind(&mut self.events).unwrap();
        let message = self.connect_message();
        for address in self.peer_discovery.iter() {
            if address == self.network.address() {
                continue
            }
            self.network.send_to(&mut self.events,
                                 address,
                                 message.clone()).unwrap();
        }
    }

    pub fn run(&mut self) {
        self.initialize();
        loop {
            match self.events.poll() {
                Event::Incoming(message) => {
                    self.handle(message);
                },
                Event::Internal(_) => {

                },
                Event::Timeout(_) => {

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

    fn handle(&mut self, message: Message) {
        // TODO: check message header (network id, protocol version)
        match message.message_type() {
            Connect::MESSAGE_TYPE => self.handle_connect(message),
            Propose::MESSAGE_TYPE => self.handle_propose(message),
            Prevote::MESSAGE_TYPE => self.handle_prevote(message),
          Precommit::MESSAGE_TYPE => self.handle_precommit(message),
             Commit::MESSAGE_TYPE => self.handle_commit(message),
            _ => {
                // TODO: unrecognized message type
            }
        }
    }

    fn handle_connect(&mut self, message: Message) {
        let public_key = message.public_key().clone();
        let address = Connect::from_raw(&message).socket_address();
        self.state.add_validator(public_key, address);
    }

    fn handle_propose(&mut self, message: Message) {
        let propose = Propose::from_raw(&message);
    }

    fn handle_prevote(&mut self, message: Message) {

    }

    fn handle_precommit(&mut self, message: Message) {

    }

    fn handle_commit(&mut self, message: Message) {

    }

    fn send_to(&mut self, address: &net::SocketAddr, message: Message) {
        self.network.send_to(&mut self.events, address, message).unwrap();
    }

    fn broadcast(&mut self, message: Message) {
        for address in self.state.validators().values() {
            if address == self.network.address() {
                continue
            }
            self.network.send_to(&mut self.events,
                                 address,
                                 message.clone()).unwrap();
        }
    }

    fn connect_message(&self) -> Message {
        Connect::new(self.network.address(), get_time(),
                     &self.public_key, &self.secret_key)
    }
}
