use std::net::SocketAddr;
use std::collections::HashMap;
use std::cmp::min;
use std::marker::PhantomData;

use time::Duration;
use rand::{thread_rng, Rng};

use exonum::crypto::{gen_keypair, gen_keypair_from_seed, Seed, PublicKey, SecretKey};
use exonum::node::Configuration;
use exonum::events::{NetworkConfiguration, EventsConfiguration};
use exonum::events::{Reactor, Events, Event, Network, InternalEvent};
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
    pub max_incoming_connections: usize,
    pub max_outgoing_connections: usize,
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
    pub propose_timeout: u32,
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
            propose_timeout: 200,
            network: NetworkConfig {
                max_incoming_connections: 128,
                max_outgoing_connections: 128,
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
            propose_timeout: self.propose_timeout,
            network: NetworkConfiguration {
                listen_address: validator.address,
                max_incoming_connections: self.network.max_incoming_connections,
                max_outgoing_connections: self.network.max_outgoing_connections,
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
