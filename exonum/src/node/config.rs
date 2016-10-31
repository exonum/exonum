use std::net::SocketAddr;

use super::super::crypto::{gen_keypair, gen_keypair_from_seed, Seed, PublicKey, SecretKey};
use super::Configuration;
use super::super::events::{NetworkConfiguration, EventsConfiguration};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListenerConfig {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConsensusConfig {
    pub round_timeout: u32,
    pub propose_timeout: u32,
    pub status_timeout: u32,
    pub peers_timeout: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenesisConfig {
    pub validators: Vec<ListenerConfig>,
    pub consensus: ConsensusConfig,
    pub network: NetworkConfiguration,
}

impl ListenerConfig {
    pub fn gen_from_seed(seed: &Seed, addr: SocketAddr) -> ListenerConfig {
        let keys = gen_keypair_from_seed(seed);
        ListenerConfig {
            public_key: keys.0,
            secret_key: keys.1,
            address: addr,
        }
    }

    pub fn gen(addr: SocketAddr) -> ListenerConfig {
        let keys = gen_keypair();
        ListenerConfig {
            public_key: keys.0,
            secret_key: keys.1,
            address: addr,
        }
    }
}

impl GenesisConfig {
    pub fn gen(validators_count: u8, port: Option<u16>) -> GenesisConfig {
        let mut pairs = Vec::new();
        let port = port.unwrap_or(7000);
        for i in 0..validators_count {
            let addr = format!("127.0.0.1:{}", port + i as u16).parse().unwrap();
            let pair = ListenerConfig::gen_from_seed(&Seed::from_slice(&[i; 32]).unwrap(),
                                                     addr);
            pairs.push(pair);
        }

        GenesisConfig {
            validators: pairs,
            consensus: ConsensusConfig {
                round_timeout: 3000,
                status_timeout: 5000,
                peers_timeout: 10000,
                propose_timeout: 300,
            },
            network: NetworkConfiguration {
                max_incoming_connections: 128,
                max_outgoing_connections: 128,
                tcp_keep_alive: None,
                tcp_nodelay: false,
                tcp_reconnect_timeout: 5000,
                tcp_reconnect_timeout_max: 600000,
            },
        }
    }

    pub fn gen_node_configuration(self,
                                 idx: usize,
                                 known_peers: Vec<::std::net::SocketAddr>)
                                 -> Configuration {
        let listener = self.validators[idx].clone();
        let validators: Vec<_> = self.validators
            .iter()
            .map(|v| v.public_key)
            .collect();

        Configuration {
            listener: listener,
            consensus: self.consensus,
            network: self.network,
            events: EventsConfiguration::new(),
            peer_discovery: known_peers,
            validators: validators,
        }
    }
}
