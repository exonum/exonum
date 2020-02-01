// Copyright 2020 The Exonum Team
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

use exonum::{
    blockchain::{config::GenesisConfigBuilder, ConsensusConfig, ValidatorKeys},
    keys::Keys,
    merkledb::TemporaryDB,
};
use exonum_explorer_service::ExplorerFactory;
use exonum_node::{NodeApiConfig, NodeBuilder, NodeConfig};
use exonum_rust_runtime::{DefaultInstance, RustRuntime, ServiceFactory};
use exonum_system_api::SystemApiPlugin;

use exonum_cryptocurrency::contracts::CryptocurrencyService;

fn node_config() -> (NodeConfig, Keys) {
    let keys = Keys::random();
    let validator_keys = vec![ValidatorKeys::new(keys.consensus_pk(), keys.service_pk())];
    let consensus = ConsensusConfig::default().with_validator_keys(validator_keys);

    let api_address = "0.0.0.0:8000".parse().unwrap();
    let api_cfg = NodeApiConfig {
        public_api_address: Some(api_address),
        ..Default::default()
    };

    let peer_address = "0.0.0.0:2000";
    let node_config = NodeConfig {
        listen_address: peer_address.parse().unwrap(),
        consensus,
        external_address: peer_address.to_owned(),
        network: Default::default(),
        connect_list: Default::default(),
        api: api_cfg,
        mempool: Default::default(),
        thread_pool_size: Default::default(),
    };
    (node_config, keys)
}

fn main() {
    exonum::helpers::init_logger().unwrap();
    let (node_cfg, node_keys) = node_config();
    let artifact_id = CryptocurrencyService.artifact_id();
    let genesis_cfg = GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone())
        .with_artifact(ExplorerFactory.artifact_id())
        .with_instance(ExplorerFactory.default_instance())
        .with_artifact(artifact_id.clone())
        .with_instance(artifact_id.into_default_instance(101, "cryptocurrency"))
        .build();

    println!("Creating database in temporary dir...");
    let db = TemporaryDB::new();
    let node = NodeBuilder::new(db, node_cfg, genesis_cfg, node_keys)
        .with_plugin(SystemApiPlugin)
        .with_runtime_fn(|channel| {
            RustRuntime::builder()
                .with_factory(CryptocurrencyService)
                .with_factory(ExplorerFactory)
                .build(channel.endpoints_sender())
        })
        .build();

    println!("Starting a single node...");
    println!("Blockchain is ready for transactions!");
    node.run().unwrap();
}
