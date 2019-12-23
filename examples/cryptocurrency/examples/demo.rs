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

use exonum::{
    blockchain::{config::GenesisConfigBuilder, ConsensusConfig, ValidatorKeys},
    keys::Keys,
    node::{Node, NodeApiConfig, NodeConfig},
    runtime::RuntimeInstance,
};
use exonum_cryptocurrency::contracts::CryptocurrencyService;
use exonum_rust_runtime::ServiceFactory;

fn node_config() -> NodeConfig {
    let (consensus_public_key, consensus_secret_key) = exonum::crypto::gen_keypair();
    let (service_public_key, service_secret_key) = exonum::crypto::gen_keypair();

    let consensus = ConsensusConfig {
        validator_keys: vec![ValidatorKeys {
            consensus_key: consensus_public_key,
            service_key: service_public_key,
        }],
        ..ConsensusConfig::default()
    };

    let api_address = "0.0.0.0:8000".parse().unwrap();
    let api_cfg = NodeApiConfig {
        public_api_address: Some(api_address),
        ..Default::default()
    };

    let peer_address = "0.0.0.0:2000";

    NodeConfig {
        listen_address: peer_address.parse().unwrap(),
        consensus,
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
        ),
    }
}

fn main() {
    exonum::helpers::init_logger().unwrap();
    let external_runtimes: Vec<RuntimeInstance> = vec![];
    let service = CryptocurrencyService;
    let artifact_id = service.artifact_id();
    let services = vec![service.into()];
    let node_config = node_config();
    let genesis_config = GenesisConfigBuilder::with_consensus_config(node_config.consensus.clone())
        .with_artifact(artifact_id.clone())
        .with_instance(artifact_id.into_default_instance(1, "cryptocurrency"))
        .build();

    println!("Creating database in temporary dir...");
    let node = Node::new(
        TemporaryDB::new(),
        external_runtimes,
        services,
        node_config,
        genesis_config,
        None,
    );
    println!("Starting a single node...");
    println!("Blockchain is ready for transactions!");
    node.run().unwrap();
}
