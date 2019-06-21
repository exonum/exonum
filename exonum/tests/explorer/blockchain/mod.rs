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
    blockchain::{Blockchain, ExecutionError, ExecutionResult, InstanceCollection, Schema},
    crypto::{self, PublicKey, SecretKey},
    helpers::generate_testnet_config,
    impl_service_dispatcher,
    messages::{AnyTx, Message, ServiceInstanceId, Signed},
    node::ApiSender,
    runtime::rust::{RustArtifactSpec, Service, ServiceFactory, TransactionContext},
};
use exonum_merkledb::{ObjectHash, TemporaryDB};
use futures::sync::mpsc;
use semver::Version;

pub const SERVICE_ID: ServiceInstanceId = 4;

mod proto;

#[derive(Debug)]
struct MyService;

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

#[service_interface]
pub trait ExplorerTransactions {
    fn create_wallet(&self, context: TransactionContext, arg: CreateWallet) -> ExecutionResult;
    fn transfer(&self, context: TransactionContext, arg: Transfer) -> ExecutionResult;
}

impl ExplorerTransactions for MyService {
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
        panic!("oops");
    }
}

impl_service_dispatcher!(MyService, ExplorerTransactions);

impl Service for MyService {}

impl ServiceFactory for MyService {
    fn artifact(&self) -> RustArtifactSpec {
        RustArtifactSpec {
            name: "my-service".into(),
            version: Version::new(1, 0, 0),
        }
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(Self)
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
    Blockchain::new(
        TemporaryDB::new(),
        vec![InstanceCollection::new(MyService).with_instance(SERVICE_ID, "my-service", ())],
        config.genesis,
        service_keypair,
        ApiSender(mpsc::unbounded().0),
        mpsc::channel(0).0,
    )
}

/// Simplified compared to real life / testkit, but we don't need to test *everything*
/// here.
pub fn create_block(blockchain: &mut Blockchain, transactions: Vec<Signed<AnyTx>>) {
    use exonum::helpers::{Round, ValidatorId};
    use exonum::messages::{Precommit, Propose};
    use std::time::SystemTime;

    let tx_hashes: Vec<_> = transactions.iter().map(Signed::object_hash).collect();
    let height = blockchain.last_block().height().next();

    let fork = blockchain.fork();
    {
        let mut schema = Schema::new(&fork);
        for tx in transactions {
            schema.add_transaction_into_pool(tx.clone())
        }
    }
    blockchain.merge(fork.into_patch()).unwrap();

    let (block_hash, patch) = blockchain.create_patch(ValidatorId(0), height, &tx_hashes);
    let (consensus_public_key, consensus_secret_key) = consensus_keys();

    let propose = Message::concrete(
        Propose::new(
            ValidatorId(0),
            height,
            Round::first(),
            &blockchain.last_hash(),
            &tx_hashes,
        ),
        consensus_public_key,
        &consensus_secret_key,
    );
    let precommit = Message::concrete(
        Precommit::new(
            ValidatorId(0),
            propose.height(),
            propose.round(),
            &propose.object_hash(),
            &block_hash,
            SystemTime::now().into(),
        ),
        consensus_public_key,
        &consensus_secret_key,
    );

    blockchain
        .commit(&patch, block_hash, vec![precommit].into_iter())
        .unwrap();
}
