// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use exonum_merkledb::TemporaryDB;

use exonum::{keys::Keys,
    blockchain::{GenesisConfig, InstanceCollection, ValidatorKeys},
    node::{Node, NodeApiConfig, NodeConfig},
};
use exonum_cryptocurrency::contracts::CryptocurrencyService;

fn node_config() -> NodeConfig {
    let (consensus_public_key, consensus_secret_key) = exonum::crypto::gen_keypair();
    let (service_public_key, service_secret_key) = exonum::crypto::gen_keypair();
    let (identity_public_key, identity_secret_key) = exonum::crypto::kx::gen_keypair();

    let validator_keys = ValidatorKeys {
        consensus_key: consensus_public_key,
        service_key: service_public_key,
        identity_key: identity_public_key,
    };
    let genesis = GenesisConfig::new(vec![validator_keys].into_iter());

    let api_address = "0.0.0.0:8000".parse().unwrap();
    let api_cfg = NodeApiConfig {
        public_api_address: Some(api_address),
        ..Default::default()
    };

    let peer_address = "0.0.0.0:2000";

    NodeConfig {
        listen_address: peer_address.parse().unwrap(),
        genesis,
        external_address: peer_address.to_owned(),
        network: Default::default(),
        connect_list: Default::default(),
        api: api_cfg,
        mempool: Default::default(),
        services_configs: Default::default(),
        database: Default::default(),
        thread_pool_size: Default::default(),
        master_key_path: Default::default(),
        keys: Keys::from_keys(
            consensus_public_key,
            consensus_secret_key,
            service_public_key,
            service_secret_key,
            identity_public_key,
            identity_secret_key,
        ),
    }
}

fn main() {
    exonum::helpers::init_logger().unwrap();

    println!("Creating database in temporary dir...");
    let node = Node::new(
        TemporaryDB::new(),
        vec![InstanceCollection::new(CryptocurrencyService)],
        node_config(),
        None,
    );
    println!("Starting a single node...");
    println!("Blockchain is ready for transactions!");
    node.run().unwrap();
}
