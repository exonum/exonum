use std::net::SocketAddr;
use std::collections::HashMap;
use std::cmp::min;
use std::marker::PhantomData;

use time::Duration;
use rand::{thread_rng, Rng};

use exonum::crypto::{gen_keypair, gen_keypair_from_seed, Seed, PublicKey, SecretKey};
use exonum::node::Configuration;
use exonum::events::{NetworkConfiguration, EventsConfiguration};
use exonum::events::{Reactor, Events, Event, NodeTimeout, Network};
use exonum::blockchain::Blockchain;
use exonum::messages::{Any, Message, Connect, RawMessage};

use timestamping::TimestampTx;

use super::TimestampingTxGenerator;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Listener {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub address: SocketAddr,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub max_connections: usize,
    pub listener: Option<Listener>,
    pub tcp_nodelay: bool,
    pub tcp_keep_alive: Option<u32>,
    pub tcp_reconnect_timeout: u64,
    pub tcp_reconnect_timeout_max: u64,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct TestNodeConfig {
    pub validators: Vec<Listener>,
    pub round_timeout: u32,
    pub status_timeout: u32,
    pub peers_timeout: u32,
    pub network: NetworkConfig,
}

impl Listener {
    pub fn gen_from_seed(seed: &Seed, addr: SocketAddr) -> Listener {
        let keys = gen_keypair_from_seed(seed);
        Listener {
            public_key: keys.0.clone(),
            secret_key: keys.1.clone(),
            address: addr,
        }
    }

    pub fn gen(addr: SocketAddr) -> Listener {
        let keys = gen_keypair();
        Listener {
            public_key: keys.0.clone(),
            secret_key: keys.1.clone(),
            address: addr,
        }
    }
}

impl TestNodeConfig {
    pub fn gen(validators_count: u8) -> TestNodeConfig {
        let mut pairs = Vec::new();
        for i in 0..validators_count {
            let addr = format!("127.0.0.1:{}", 7000 + i as u32).parse().unwrap();
            let pair = Listener::gen_from_seed(&Seed::from_slice(&vec![i; 32]).unwrap(), addr);
            pairs.push(pair);
        }

        TestNodeConfig {
            validators: pairs,
            round_timeout: 1000,
            status_timeout: 5000,
            peers_timeout: 10000,
            network: NetworkConfig {
                max_connections: 256,
                tcp_keep_alive: None,
                tcp_nodelay: false,
                tcp_reconnect_timeout: 5000,
                tcp_reconnect_timeout_max: 600000,
                listener: None,
            },
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
                max_connections: self.network.max_connections,
                tcp_nodelay: self.network.tcp_nodelay,
                tcp_keep_alive: self.network.tcp_keep_alive,
                tcp_reconnect_timeout: self.network.tcp_reconnect_timeout,
                tcp_reconnect_timeout_max: self.network.tcp_reconnect_timeout_max,
            },
            events: EventsConfiguration::new(),
            peer_discovery: known_peers,
            validators: validators,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TxGeneratorConfiguration {
    pub network: NetworkConfig,
    pub tx_timeout: u32,
    pub tx_package_size: usize,
}

pub struct TxGeneratorNode<'a, B: Blockchain> {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub events: Box<Reactor>,

    pub our_connect: Connect,
    pub peers: HashMap<PublicKey, Connect>,

    pub tx_timeout: u32,
    pub tx_package_size: usize,
    pub tx_remaining: usize,
    pub tx_receivers: Vec<SocketAddr>,
    pub tx_gen: &'a mut TimestampingTxGenerator,

    _b: PhantomData<B>
}

impl<'a, B: Blockchain> TxGeneratorNode<'a, B> {
    pub fn new(cfg: TxGeneratorConfiguration, receivers: Vec<SocketAddr>, remaining: usize, gen: &'a mut TimestampingTxGenerator) -> TxGeneratorNode<'a, B> {
        let listener = cfg.network.listener.unwrap();
        let network = NetworkConfiguration {
            listen_address: listener.address,
            max_connections: cfg.network.max_connections,
            tcp_nodelay: cfg.network.tcp_nodelay,
            tcp_keep_alive: cfg.network.tcp_keep_alive,
            tcp_reconnect_timeout: cfg.network.tcp_reconnect_timeout,
            tcp_reconnect_timeout_max: cfg.network.tcp_reconnect_timeout_max,
        };

        let events = EventsConfiguration::new();
        let network = Network::with_config(network);
        let reactor = Box::new(Events::with_config(events, network).unwrap()) as Box<Reactor>;

        let connect_msg = Connect::new(&listener.public_key,
                                       listener.address,
                                       reactor.get_time(),
                                       &listener.secret_key);

        TxGeneratorNode {
            public_key: listener.public_key,
            secret_key: listener.secret_key,
            events: reactor,
            our_connect: connect_msg,
            peers: HashMap::new(),
            tx_timeout: cfg.tx_timeout,
            tx_package_size: cfg.tx_package_size,
            tx_receivers: receivers,
            tx_remaining: remaining,
            tx_gen: gen,
            _b: PhantomData
        }
    }

    pub fn initialize(&mut self) {
        info!("Starting transaction sending...");
        self.events.bind().unwrap();

        let connect = self.our_connect.clone();
        for address in self.tx_receivers.clone() {
            if address == self.events.address() {
                continue;
            }
            self.send_to_addr(&address, connect.raw());
        }
        self.add_timeout();
    }

    pub fn run(&mut self) {
        self.initialize();
        loop {
            match self.events.poll() {
                Event::Incoming(message) => {
                    self.handle_message(message);
                }
                Event::Internal(_) => {}
                Event::Timeout(timeout) => {
                    if !self.handle_timeout(timeout) {
                        break;
                    }
                }
                Event::Error(_) => {}
                Event::Connected(_) => {}
                Event::Disconnected(_) => {}
                Event::Terminate => break,
            }
        }
    }

    pub fn send_to_peer(&mut self, public_key: PublicKey, message: &RawMessage) {
        if let Some(conn) = self.peers.get(&public_key) {
            self.events.send_to(&conn.addr(), message.clone());
        } else {
            warn!("attempt to send data to a peer: {:?} that is not connected",
                  public_key);
        }
    }

    pub fn send_to_addr(&mut self, address: &SocketAddr, message: &RawMessage) {
        self.events.send_to(address, message.clone());
    }

    // TODO: use Into<RawMessage>
    pub fn broadcast(&mut self, message: &RawMessage) {
        for conn in self.peers.values() {
            self.events.send_to(&conn.addr(), message.clone());
        }
    }

    fn add_timeout(&mut self) {
        let time = self.events.get_time() + Duration::milliseconds(self.tx_timeout as i64);
        self.events.add_timeout(NodeTimeout::PeerExchange, time);
    }

    fn handle_timeout(&mut self, _: NodeTimeout) -> bool {
        if self.send_transactions() {
            self.add_timeout();
            true
        } else {
            info!("Transactions sending finished");
            false
        }
    }

    fn handle_message(&mut self, raw: RawMessage) {
        match Any::from_raw(raw).unwrap() {
            Any::Connect(msg) => self.handle_connect(msg),
            Any::Status(_) => {}
            Any::Transaction(message) => self.handle_tx(message),
            Any::Consensus(_) => {}
            Any::Request(_) => {}
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

    fn handle_tx(&mut self, _: B::Transaction) {}

    fn send_transactions(&mut self) -> bool {
        let count = min(self.tx_remaining, self.tx_package_size);
        let receivers = self.tx_receivers.clone();
        let transactions = self.tx_gen
            .map(|x| (*thread_rng().choose(receivers.as_slice()).unwrap(), x))
            .take(count)
            .collect::<Vec<(SocketAddr, TimestampTx)>>();

        for entry in transactions {
            self.send_to_addr(&entry.0, entry.1.raw());
        }
        self.tx_remaining -= count;

        debug!("{:?}, There are {} transactions in the pool", self.events.get_time(), self.tx_remaining);
        self.tx_remaining != 0
    }
}
