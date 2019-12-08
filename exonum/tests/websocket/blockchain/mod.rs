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
    blockchain::config::GenesisConfigBuilder,
    crypto::{Hash, PublicKey},
    helpers,
    node::{ApiSender, Node},
    runtime::{
        rust::{CallContext, Service, ServiceFactory},
        BlockchainData, ExecutionError, InstanceId, RuntimeInstance,
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
    pub name: String,
}

impl CreateWallet {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Transfer")]
pub struct Transfer {
    pub to: PublicKey,
    pub amount: u64,
}

impl Transfer {
    pub fn new(to: PublicKey, amount: u64) -> Self {
        Self { to, amount }
    }
}

#[derive(Debug, ExecutionFail)]
pub enum Error {
    /// Not allowed!
    NotAllowed = 0,
}

#[exonum_interface]
pub trait Transactions<Ctx> {
    fn create_wallet(&self, ctx: Ctx, arg: CreateWallet) -> _;
    fn transfer(&self, ctx: Ctx, arg: Transfer) -> _;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "ws-test", proto_sources = "exonum::proto::schema")]
#[service_dispatcher(implements("Transactions"))]
struct MyService;

impl Transactions<CallContext<'_>> for MyService {
    type Output = Result<(), ExecutionError>;

    fn create_wallet(&self, _ctx: CallContext<'_>, arg: CreateWallet) -> Self::Output {
        if arg.name.starts_with("Al") {
            Ok(())
        } else {
            Err(Error::NotAllowed.into())
        }
    }

    fn transfer(&self, _ctx: CallContext<'_>, _arg: Transfer) -> Self::Output {
        panic!("oops")
    }
}

impl Service for MyService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
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

    let external_runtimes: Vec<RuntimeInstance> = vec![];
    let service = MyService;
    let artifact = service.artifact_id();
    let genesis_config = GenesisConfigBuilder::with_consensus_config(node_cfg.consensus.clone())
        .with_artifact(artifact.clone())
        .with_instance(artifact.into_default_instance(SERVICE_ID, "my-service"))
        .build();
    let services = vec![service.into()];

    let node = Node::new(
        TemporaryDB::new(),
        external_runtimes,
        services,
        node_cfg,
        genesis_config,
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
