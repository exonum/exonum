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

//! Simplified node emulation for testing websockets.

use std::{
    borrow::Cow,
    net::SocketAddr,
    thread::{self, JoinHandle},
};

use exonum::{
    blockchain::{
        ExecutionError, ExecutionResult, Service, Transaction, TransactionContext, TransactionSet,
    },
    crypto::{Hash, PublicKey},
    helpers,
    messages::RawTransaction,
    node::{ApiSender, Node},
};

use exonum_merkledb::{Snapshot, TemporaryDB};

mod proto;

pub const SERVICE_ID: u16 = 0;

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::CreateWallet")]
pub struct CreateWallet {
    pub pubkey: PublicKey,
    pub name: String,
}

impl CreateWallet {
    pub fn new(pubkey: &PublicKey, name: &str) -> Self {
        Self {
            pubkey: *pubkey,
            name: name.to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Transfer")]
pub struct Transfer {
    pub from: PublicKey,
    pub to: PublicKey,
    pub amount: u64,
}

impl Transfer {
    pub fn new(from: &PublicKey, to: &PublicKey, amount: u64) -> Self {
        Self {
            from: *from,
            to: *to,
            amount,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
pub enum Transactions {
    CreateWallet(CreateWallet),
    Transfer(Transfer),
}

impl Transaction for CreateWallet {
    fn execute(&self, _: TransactionContext) -> ExecutionResult {
        if self.name.starts_with("Al") {
            Ok(())
        } else {
            Err(ExecutionError::with_description(
                1,
                "Not allowed".to_string(),
            ))
        }
    }
}

impl Transaction for Transfer {
    fn execute(&self, _: TransactionContext) -> ExecutionResult {
        panic!("oops")
    }
}

struct MyService;

impl Service for MyService {
    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn service_name(&self) -> &str {
        "my-service"
    }

    fn state_hash(&self, _: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        Transactions::tx_from_raw(raw).map(Transactions::into)
    }
}

pub struct RunHandle {
    pub node_thread: JoinHandle<()>,
    pub api_tx: ApiSender,
}

pub fn run_node(listen_port: u16, pub_api_port: u16) -> RunHandle {
    let mut node_cfg = helpers::generate_testnet_config(1, listen_port).remove(0);
    node_cfg.api.public_api_address = Some(
        format!("127.0.0.1:{}", pub_api_port)
            .parse::<SocketAddr>()
            .unwrap(),
    );
    let service = Box::new(MyService);
    let node = Node::new(TemporaryDB::new(), vec![service], node_cfg, None);
    let api_tx = node.channel();
    RunHandle {
        node_thread: thread::spawn(move || {
            node.run().unwrap();
        }),
        api_tx,
    }
}
