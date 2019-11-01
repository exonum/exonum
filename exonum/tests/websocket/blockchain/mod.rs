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
    blockchain::InstanceCollection,
    crypto::{Hash, PublicKey},
    helpers,
    node::{ApiSender, Node},
    runtime::{
        rust::{CallContext, Service},
        InstanceDescriptor, InstanceId, Runtime,
    },
};
use exonum_merkledb::{Snapshot, TemporaryDB};
use exonum_proto::ProtobufConvert;

use std::{
    net::SocketAddr,
    thread::{self, JoinHandle},
    time::Duration,
};

mod proto;

pub const SERVICE_ID: InstanceId = 118;

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::CreateWallet")]
pub struct CreateWallet {
    pub pubkey: PublicKey,
    pub name: String,
}

impl CreateWallet {
    pub fn new(pubkey: PublicKey, name: &str) -> Self {
        Self {
            pubkey,
            name: name.to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Transfer")]
pub struct Transfer {
    pub from: PublicKey,
    pub to: PublicKey,
    pub amount: u64,
}

impl Transfer {
    pub fn new(from: PublicKey, to: PublicKey, amount: u64) -> Self {
        Self { from, to, amount }
    }
}

#[derive(Debug, IntoExecutionError)]
pub enum Error {
    /// Not allowed
    NotAllowed = 0,
}

#[exonum_service]
pub trait MyServiceInterface {
    fn create_wallet(&self, context: CallContext<'_>, arg: CreateWallet) -> Result<(), Error>;
    fn transfer(&self, context: CallContext<'_>, arg: Transfer) -> Result<(), Error>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "ws-test",
    proto_sources = "exonum::proto::schema",
    implements("MyServiceInterface")
)]
struct MyService;

impl MyServiceInterface for MyService {
    fn create_wallet(&self, _context: CallContext<'_>, arg: CreateWallet) -> Result<(), Error> {
        if arg.name.starts_with("Al") {
            Ok(())
        } else {
            Err(Error::NotAllowed)
        }
    }
    fn transfer(&self, _context: CallContext<'_>, _arg: Transfer) -> Result<(), Error> {
        panic!("oops")
    }
}

impl Service for MyService {
    fn state_hash(&self, _instance: InstanceDescriptor<'_>, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
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

    let external_runtimes: Vec<(u32, Box<dyn Runtime>)> = vec![];
    let services =
        vec![InstanceCollection::new(MyService).with_instance(SERVICE_ID, "my-service", ())];

    let node = Node::new(
        TemporaryDB::new(),
        external_runtimes,
        services,
        node_cfg,
        None,
    );

    let api_tx = node.channel();
    let handle = RunHandle {
        node_thread: thread::spawn(move || {
            node.run().unwrap();
        }),
        api_tx,
    };
    // Wait until the node has fully started.
    thread::sleep(Duration::from_secs(1));
    handle
}
