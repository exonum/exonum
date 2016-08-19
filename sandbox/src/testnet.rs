use std::path::Path;
use std::fs;
use std::error::Error;
use std::io::prelude::*;
use std::fs::File;
use std::net::SocketAddr;
use std::collections::HashMap;
use std::cmp::min;

use time::{Duration};
use serde::{Serialize, Deserialize};
use toml;
use toml::Encoder;

use exonum::crypto::{gen_keypair_from_seed, Seed, PublicKey, SecretKey};
use exonum::node::{Configuration};
use exonum::events::{NetworkConfiguration, EventsConfiguration};
use exonum::events::{Reactor, Events, Event, Timeout, Network};
use exonum::storage::{Blockchain};
use exonum::messages::{Any, Message, Connect, RawMessage};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestnetValidator {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub address: SocketAddr,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestnetConfiguration {
    pub validators: Vec<TestnetValidator>,
    pub round_timeout: u32,
    pub status_timeout: u32,
    pub peers_timeout: u32,
    pub max_incoming_connections: usize,
    pub max_outgoing_connections: usize,
}

impl TestnetConfiguration {
    pub fn gen(validators_count: u8) -> TestnetConfiguration {
        let mut pairs = Vec::new();
        for i in 0..validators_count {
            let keys = gen_keypair_from_seed(&Seed::from_slice(&vec![i; 32]).unwrap());
            let addr = format!("127.0.0.1:{}", 7000 + i as u32).parse().unwrap();
            let pair = TestnetValidator {
                public_key: keys.0.clone(),
                secret_key: keys.1.clone(),
                address: addr,
            };
            pairs.push(pair);
        }

        TestnetConfiguration {
            validators: pairs,
            round_timeout: 1000,
            status_timeout: 5000,
            peers_timeout: 10000,
            max_incoming_connections: 128,
            max_outgoing_connections: 128,
        }
    }

    pub fn to_node_configuration(&self,
                             idx: usize,
                             known_peers: Vec<::std::net::SocketAddr>)
                             -> Configuration {
        let validator = self.validators[idx].clone();
        let validators: Vec<_> = self.validators
            .iter()
            .map(|v| v.public_key)
            .collect();

        Configuration {
            public_key: validator.public_key,
            secret_key: validator.secret_key,
            round_timeout: self.round_timeout,
            status_timeout: self.status_timeout,
            peers_timeout: self.peers_timeout,
            network: NetworkConfiguration {
                listen_address: validator.address,
                max_incoming_connections: self.max_incoming_connections,
                max_outgoing_connections: self.max_outgoing_connections,
            },
            events: EventsConfiguration::new(),
            peer_discovery: known_peers,
            validators: validators,
        }
    }
}

pub trait ConfigEntry : Serialize {
    type Entry: Deserialize;

    fn from_file(path: &Path) -> Result<Self::Entry, Box<Error>> {
        let mut file = File::open(path)?;
        let mut toml = String::new();
        file.read_to_string(&mut toml)?;
        let cfg = toml::decode_str(&toml);
        return Ok(cfg.unwrap());
    }

    fn save_to_file(&self, path: &Path) -> Result<(), Box<Error>> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).unwrap();
        }

        let mut e = Encoder::new();
        self.serialize(&mut e).unwrap();
        let mut file = File::create(path).unwrap();
        file.write_all(toml::encode_str(&e.toml).as_bytes())?;

        Ok(())
    }
}

impl ConfigEntry for TestnetConfiguration {
    type Entry = Self;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TxGeneratorConfiguration {
    pub network: TestnetValidator,
    pub tx_timeout: u32,
    pub tx_package_size: usize
}

impl TxGeneratorConfiguration {
    pub fn new() -> TxGeneratorConfiguration {
        let keys = gen_keypair_from_seed(&Seed::from_slice(&vec![188; 32]).unwrap());

        TxGeneratorConfiguration {
            network: TestnetValidator {
                public_key: keys.0,
                secret_key: keys.1,
                address: "127.0.0.1:8000".parse().unwrap()
            },
            tx_timeout: 1000,
            tx_package_size: 1000
        }
    }
}

pub struct TxGeneratorNode<B: Blockchain> {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub events: Box<Reactor>,

    pub our_connect: Connect,
    pub peers: HashMap<PublicKey, Connect>,
    pub tx_queue: Vec<(SocketAddr, B::Transaction)>,
    pub tx_timeout: u32,
    pub tx_package_size: usize
}

impl<B: Blockchain> TxGeneratorNode<B> {
    pub fn new(cfg: TxGeneratorConfiguration) -> TxGeneratorNode<B> {
        let network = NetworkConfiguration {
                listen_address: cfg.network.address,
                max_incoming_connections: 128,
                max_outgoing_connections: 128,
        };
        let events = EventsConfiguration::new();
        let network = Network::with_config(network);
        let reactor =
            Box::new(Events::with_config(events, network).unwrap()) as Box<Reactor>;

        let connect = Connect::new(&cfg.network.public_key,
                                   cfg.network.address,
                                   reactor.get_time(),
                                   &cfg.network.secret_key);

        TxGeneratorNode {
            public_key: cfg.network.public_key,
            secret_key: cfg.network.secret_key,
            events: reactor,
            our_connect: connect,
            peers: HashMap::new(),
            tx_queue: Vec::new(),
            tx_timeout: cfg.tx_timeout,
            tx_package_size: cfg.tx_package_size
        }
    }

    pub fn initialize(&mut self, peer_discovery: &Vec<SocketAddr>) {
        info!("Start listening...");
        self.events.bind().unwrap();

        let connect = self.our_connect.clone();
        for address in peer_discovery.iter() {
            if address == &self.events.address() {
                continue;
            }
            self.send_to_addr(address, connect.raw());
        }
        self.add_timeout();
    }

    pub fn run(&mut self, peer_discovery: &Vec<SocketAddr>) {
        self.initialize(peer_discovery);
        loop {
            match self.events.poll() {
                Event::Incoming(message) => {
                    self.handle_message(message);
                }
                Event::Internal(_) => {}
                Event::Timeout(timeout) => {
                    self.handle_timeout(timeout);
                }
                Event::Error(_) => {}
                Event::Terminate => break,
            }
        }
    }

    pub fn append_transactions<I>(&mut self, iter: I)
        where I: IntoIterator<Item=(SocketAddr, B::Transaction)>
    {
        self.tx_queue.extend(iter);
    }

    pub fn send_to_peer(&mut self, public_key: PublicKey, message: &RawMessage) {
        if let Some(conn) = self.peers.get(&public_key) {
            self.events.send_to(&conn.addr(), message.clone()).unwrap();
        } else {
            // TODO: warning - hasn't connection with peer
        }
    }

    pub fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        self.events.send_to(address, message.clone()).unwrap();
    }

    // TODO: use Into<RawMessage>
    pub fn broadcast(&mut self, message: &RawMessage) {
        for conn in self.peers.values() {
            self.events.send_to(&conn.addr(), message.clone()).unwrap();
        }
    }

    fn add_timeout(&mut self) {
        let time = self.events.get_time() + Duration::milliseconds(self.tx_timeout as i64);
        self.events.add_timeout(Timeout::PeerExchange, time);
    }

    fn handle_timeout(&mut self, _: Timeout) {
        if self.send_transactions() {
            self.add_timeout();
        } else {
            info!("Transaction sending finished");
            self.events.shutdown();
        }
    }

    fn handle_message(&mut self, raw: RawMessage) {
        // TODO: check message headers (network id, protocol version)
        // FIXME: call message.verify method
        //     if !raw.verify() {
        //         return;
        //     }

        match Any::from_raw(raw).unwrap() {
            Any::Connect(msg) => self.handle_connect(msg),
            Any::Status(_) => {},
            Any::Transaction(message) => { self.handle_tx(message) },
            Any::Consensus(_) => {},
            Any::Request(_) => {},
        }
    }

    fn handle_connect(&mut self, msg: Connect) {
        if msg.addr() == self.our_connect.addr() {
            return;
        }
        debug!("handle connect message with {}", msg.addr());

        if self.peers.insert(*msg.pub_key(), msg.clone()).is_none() {
            let c = self.our_connect.clone();
            debug!("Establish connection with {}", msg.addr());
            self.send_to_addr(&msg.addr(), c.raw());
        }
    }

    fn handle_tx(&mut self, msg: B::Transaction) {

    }

    fn send_transactions(&mut self) -> bool {
        let to = min(self.tx_queue.len(), self.tx_package_size);
        let head = self.tx_queue
            .drain(0..to)
            .collect::<Vec<(SocketAddr, B::Transaction)>>();

        for entry in &head {
            self.send_to_addr(&entry.0, entry.1.raw());
        }

        debug!("There are {} transactions in the pool", self.tx_queue.len());
        !self.tx_queue.is_empty()
    }
}
