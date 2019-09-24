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

use exonum::{
    helpers::generate_testnet_config,
    node::{ApiSender, Node, NodeConfig},
    storage::MemoryDB,
};
use std::{
    net::SocketAddr,
    thread::{self, JoinHandle},
};

use crate::blockchain::MyService;

pub(crate) struct RunHandle {
    pub node_thread: JoinHandle<()>,
    pub api_tx: ApiSender,
}

pub(crate) fn run_node_with_message_len(
    listen_port: u16,
    pub_api_port: u16,
    max_message_len: u32,
) -> RunHandle {
    let mut node_cfg = generate_testnet_config(1, listen_port).remove(0);
    node_cfg.genesis.consensus.max_message_len = max_message_len;
    node_cfg.api.public_api_address = Some(
        format!("127.0.0.1:{}", pub_api_port)
            .parse::<SocketAddr>()
            .unwrap(),
    );
    run_node_with_cfg(node_cfg)
}

fn run_node_with_cfg(node_cfg: NodeConfig) -> RunHandle {
    let service = Box::new(MyService);
    let node = Node::new(MemoryDB::new(), vec![service], node_cfg, None);
    let api_tx = node.channel();
    RunHandle {
        node_thread: thread::spawn(move || {
            node.run().unwrap();
        }),
        api_tx,
    }
}
