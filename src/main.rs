extern crate exonum;

use exonum::blockchain::{Blockchain, Service, GenesisConfig};
use exonum::node::{Node, NodeConfig, NodeApiConfig};
use exonum::storage::MemoryDB;
use exonum::crypto::gen_keypair;

fn main() {

    let services: Vec<Box<Service>> = vec![
        //Box::new(CurrencyService::new()),
    ];

    let db = MemoryDB::new();
    let blockchain = Blockchain::new(db, services);

    let (public_key, secret_key) = gen_keypair();
    let genesis = GenesisConfig::new(vec![public_key].into_iter());

    let peer = "0.0.0.0:2000".parse().unwrap();
    let api = "0.0.0.0:8000".parse().unwrap();

    let api_cfg = NodeApiConfig {
        enable_blockchain_explorer: true,
        public_api_address: Some(api),
        private_api_address: None,
    };

    let node_cfg = NodeConfig {
        listen_address: peer,
        peers: vec![peer],
        public_key,
        secret_key,
        genesis,
        network: Default::default(),
        whitelist: Default::default(),
        api: api_cfg,
    };

    let mut node = Node::new(blockchain, node_cfg);
    node.run().unwrap();
}
