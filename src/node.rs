use std::{net};

use super::signature::{PublicKey, SecretKey};
use super::events::{Events, Event, EventsConfiguration};
use super::network::{Network, NetworkConfiguration};
use super::message::{Message, MessageData, MessageHeader};
use super::state::{State};

const CONNECT_MESSAGE : u16 = 0;
const PREVOTE_MESSAGE : u16 = 1;

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
        match message.header().message_type() {
            CONNECT_MESSAGE => {
                let public_key = message.header().public_key().clone();
                let address = ::std::str::from_utf8(message.payload())
                                          .unwrap().parse().unwrap();
                info!("add validator {}", address);
                let message = self.prevote_message();
                self.send_to(&address, message);
                self.state.add_validator(public_key, address);
            },
            PREVOTE_MESSAGE => {
                let new_height = ::std::str::from_utf8(message.payload())
                                            .unwrap().parse().unwrap();
                if !self.state.validate_height(new_height) {
                    info!("INVALID HEIGHT {} (current: {})",
                          new_height, self.state.height());
                    return
                }
                self.state.add_prevote();
                if self.state.has_consensus() {
                    self.state.new_height(new_height);
                    info!("new height {}", new_height);
                    let message = self.prevote_message();
                    self.broadcast(message);
                }
            },
            _ => {
                // TODO: undefined message error
            }
        }
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

    fn create_message(&self, message_type: u16, s: &str) -> Message {
        let mut data = MessageData::new();
        {
            let mut header = data.header_mut();
            header.set_message_type(message_type);
            header.set_length(s.len());
            header.set_public_key(&self.public_key);
        }
        data.extend(s.as_bytes());
        Message::new(data)
    }

    fn connect_message(&self) -> Message {
        self.create_message(CONNECT_MESSAGE,
                            &self.network.address().to_string())
    }

    fn prevote_message(&self) -> Message {
        self.create_message(PREVOTE_MESSAGE,
                            &(self.state.height() + 1).to_string())
    }
}
