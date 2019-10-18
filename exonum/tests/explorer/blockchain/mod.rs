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
    blockchain::{Blockchain, InstanceCollection, Schema},
    crypto::{self, Hash, PublicKey, SecretKey},
    helpers::generate_testnet_config,
    messages::Verified,
    node::ApiSender,
    runtime::{
        rust::{Service, TransactionContext},
        AnyTx, InstanceDescriptor, InstanceId, Runtime,
    },
};
use exonum_merkledb::{ObjectHash, Snapshot, TemporaryDB};
use exonum_proto_derive::protobuf_convert;
use futures::sync::mpsc;

use std::collections::BTreeMap;

pub const SERVICE_ID: InstanceId = 4;

mod proto;

#[protobuf_convert(source = "proto::CreateWallet")]
#[derive(Serialize, Deserialize, Clone, Debug, BinaryValue, ObjectHash)]
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

#[protobuf_convert(source = "proto::Transfer")]
#[derive(Serialize, Deserialize, Clone, Debug, BinaryValue, ObjectHash)]
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

#[derive(Debug, IntoExecutionError)]
pub enum Error {
    /// Not allowed
    NotAllowed = 0,
}

#[exonum_service]
pub trait ExplorerTransactions {
    fn create_wallet(&self, context: TransactionContext, arg: CreateWallet) -> Result<(), Error>;
    fn transfer(&self, context: TransactionContext, arg: Transfer) -> Result<(), Error>;
}

#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "my-service",
    artifact_version = "1.0.1",
    proto_sources = "proto",
    implements("ExplorerTransactions")
)]
struct MyService;

impl ExplorerTransactions for MyService {
    fn create_wallet(&self, _context: TransactionContext, arg: CreateWallet) -> Result<(), Error> {
        if arg.name.starts_with("Al") {
            Ok(())
        } else {
            Err(Error::NotAllowed)
        }
    }

    fn transfer(&self, _context: TransactionContext, _arg: Transfer) -> Result<(), Error> {
        panic!("oops");
    }
}

impl Service for MyService {
    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}

/// Generates a keypair from a fixed passphrase.
pub fn consensus_keys() -> (PublicKey, SecretKey) {
    const SEED_PHRASE: &[u8] = b"correct horse battery staple";
    let seed = crypto::Seed::from_slice(crypto::hash(SEED_PHRASE).as_ref()).unwrap();
    crypto::gen_keypair_from_seed(&seed)
}

/// Creates a blockchain with no blocks.
pub fn create_blockchain() -> Blockchain {
    let config = generate_testnet_config(1, 0)[0].clone();
    let service_keypair = config.service_keypair();

    let external_runtimes: Vec<(u32, Box<dyn Runtime>)> = vec![];
    let services =
        vec![InstanceCollection::new(MyService).with_instance(SERVICE_ID, "my-service", ())];

    Blockchain::new(
        TemporaryDB::new(),
        external_runtimes,
        services,
        config.consensus,
        service_keypair,
        ApiSender(mpsc::channel(0).0),
        mpsc::channel(0).0,
    )
}

/// Simplified compared to real life / testkit, but we don't need to test *everything*
/// here.
pub fn create_block(blockchain: &mut Blockchain, transactions: Vec<Verified<AnyTx>>) {
    use exonum::helpers::{Round, ValidatorId};
    use exonum::messages::{Precommit, Propose};
    use std::time::SystemTime;

    let tx_hashes: Vec<_> = transactions.iter().map(ObjectHash::object_hash).collect();
    let height = blockchain.last_block().height().next();

    let fork = blockchain.fork();
    {
        let mut schema = Schema::new(&fork);
        for tx in transactions {
            schema.add_transaction_into_pool(tx.clone())
        }
    }
    blockchain.merge(fork.into_patch()).unwrap();

    let mut tx_cache = BTreeMap::new();
    let (block_hash, patch) =
        blockchain.create_patch(ValidatorId(0), height, &tx_hashes, &mut tx_cache);
    let (consensus_public_key, consensus_secret_key) = consensus_keys();

    let propose = Verified::from_value(
        Propose::new(
            ValidatorId(0),
            height,
            Round::first(),
            blockchain.last_hash(),
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
