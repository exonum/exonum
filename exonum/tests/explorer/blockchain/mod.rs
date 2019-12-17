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

//! Simplified blockchain emulation for the `BlockchainExplorer`.

use exonum::{
    blockchain::{config::GenesisConfigBuilder, Blockchain, BlockchainBuilder, BlockchainMut},
    crypto::{self, PublicKey, SecretKey},
    helpers::generate_testnet_config,
    messages::Verified,
    node::ApiSender,
    runtime::{
        rust::{CallContext, RustRuntime, Service, ServiceFactory},
        AnyTx, ExecutionError, InstanceId,
    },
};
use exonum_derive::*;
use exonum_merkledb::{ObjectHash, TemporaryDB};
use exonum_proto::ProtobufConvert;
use futures::sync::mpsc;
use serde_derive::*;

use std::collections::BTreeMap;

pub const SERVICE_ID: InstanceId = 4;

mod proto;

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::CreateWallet")]
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

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Transfer")]
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

#[derive(Debug, ExecutionFail)]
pub enum Error {
    /// Not allowed!
    NotAllowed = 0,
}

#[exonum_interface]
pub trait ExplorerTransactions {
    fn create_wallet(
        &self,
        context: CallContext<'_>,
        arg: CreateWallet,
    ) -> Result<(), ExecutionError>;

    fn transfer(&self, context: CallContext<'_>, arg: Transfer) -> Result<(), ExecutionError>;
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(
    artifact_name = "my-service",
    artifact_version = "1.0.1",
    proto_sources = "proto"
)]
#[service_dispatcher(implements("ExplorerTransactions"))]
struct MyService;

impl ExplorerTransactions for MyService {
    fn create_wallet(
        &self,
        _context: CallContext<'_>,
        arg: CreateWallet,
    ) -> Result<(), ExecutionError> {
        if arg.name.starts_with("Al") {
            Ok(())
        } else {
            Err(Error::NotAllowed.into())
        }
    }

    fn transfer(&self, _context: CallContext<'_>, _arg: Transfer) -> Result<(), ExecutionError> {
        panic!("oops");
    }
}

impl Service for MyService {}

/// Generates a keypair from a fixed passphrase.
pub fn consensus_keys() -> (PublicKey, SecretKey) {
    const SEED_PHRASE: &[u8] = b"correct horse battery staple";
    let seed = crypto::Seed::from_slice(crypto::hash(SEED_PHRASE).as_ref()).unwrap();
    crypto::gen_keypair_from_seed(&seed)
}

/// Creates a blockchain with no blocks.
pub fn create_blockchain() -> BlockchainMut {
    let config = generate_testnet_config(1, 0)[0].clone();
    let blockchain = Blockchain::new(
        TemporaryDB::new(),
        config.service_keypair(),
        ApiSender::closed(),
    );

    let my_service = MyService;
    let my_service_artifact = my_service.artifact_id();
    let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus)
        .with_artifact(my_service_artifact.clone())
        .with_instance(my_service_artifact.into_default_instance(SERVICE_ID, "my-service"))
        .build();
    let rust_runtime = RustRuntime::new(mpsc::channel(1).0).with_factory(my_service);
    BlockchainBuilder::new(blockchain, genesis_config)
        .with_runtime(rust_runtime)
        .build()
        .unwrap()
}

/// Simplified compared to real life / testkit, but we don't need to test *everything*
/// here.
pub fn create_block(blockchain: &mut BlockchainMut, transactions: Vec<Verified<AnyTx>>) {
    use exonum::helpers::{Round, ValidatorId};
    use exonum::messages::{Precommit, Propose};
    use std::time::SystemTime;

    let tx_hashes: Vec<_> = transactions.iter().map(ObjectHash::object_hash).collect();
    let height = blockchain.as_ref().last_block().height.next();
    blockchain.add_transactions_into_pool(transactions);

    let mut tx_cache = BTreeMap::new();
    let (block_hash, patch) =
        blockchain.create_patch(ValidatorId(0).into(), height, &tx_hashes, &mut tx_cache);
    let (consensus_public_key, consensus_secret_key) = consensus_keys();

    let propose = Verified::from_value(
        Propose::new(
            ValidatorId(0),
            height,
            Round::first(),
            blockchain.as_ref().last_hash(),
            tx_hashes,
        ),
        consensus_public_key,
        &consensus_secret_key,
    );
    let precommit = Verified::from_value(
        Precommit::new(
            ValidatorId(0),
            propose.payload().height,
            propose.payload().round,
            propose.object_hash(),
            block_hash,
            SystemTime::now().into(),
        ),
        consensus_public_key,
        &consensus_secret_key,
    );

    blockchain
        .commit(patch, block_hash, vec![precommit], &mut tx_cache)
        .unwrap();
}
