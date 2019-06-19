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

use exonum::{
    blockchain::{ExecutionError, ExecutionResult, ServiceInstances},
    crypto::PublicKey,
    helpers, impl_service_dispatcher,
    node::{ApiSender, Node},
    runtime::{
        rust::{RustArtifactSpec, Service, ServiceFactory, TransactionContext},
        ServiceInstanceId,
    },
};
use exonum_merkledb::TemporaryDB;

use std::{
    net::SocketAddr,
    thread::{self, JoinHandle},
};

mod proto;

pub const SERVICE_ID: ServiceInstanceId = 118;

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

#[service_interface]
pub trait MyServiceInterface {
    fn create_wallet(&self, context: TransactionContext, arg: CreateWallet) -> ExecutionResult;
    fn transfer(&self, context: TransactionContext, arg: Transfer) -> ExecutionResult;
}

#[derive(Debug)]
struct MyService;

impl MyServiceInterface for MyService {
    fn create_wallet(&self, _context: TransactionContext, arg: CreateWallet) -> ExecutionResult {
        if arg.name.starts_with("Al") {
            Ok(())
        } else {
            Err(ExecutionError::with_description(
                1,
                "Not allowed".to_string(),
            ))
        }
    }
    fn transfer(&self, _context: TransactionContext, _arg: Transfer) -> ExecutionResult {
        panic!("oops")
    }
}

impl_service_dispatcher!(MyService, MyServiceInterface);

impl Service for MyService {}

impl ServiceFactory for MyService {
    fn artifact(&self) -> RustArtifactSpec {
        "ws-test/0.1.0".parse().unwrap()
    }
    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
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

    let node = Node::new(
        TemporaryDB::new(),
        vec![ServiceInstances::new(MyService).with_instance(SERVICE_ID, "my-service", ())],
        node_cfg,
        None,
    );

    let api_tx = node.channel();
    RunHandle {
        node_thread: thread::spawn(move || {
            node.run().unwrap();
        }),
        api_tx,
    }
}
