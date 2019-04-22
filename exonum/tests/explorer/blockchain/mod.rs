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

use futures::sync::mpsc;

use std::borrow::Cow;

use exonum::{
    blockchain::{
        Blockchain, ExecutionError, ExecutionResult, Schema, Service, Transaction,
        TransactionContext, TransactionSet,
    },
    crypto::{self, Hash, PublicKey, SecretKey},
    messages::{Message, RawTransaction, Signed},
    node::ApiSender,
};

use exonum_merkledb::{Snapshot, TemporaryDB};

pub const SERVICE_ID: u16 = 0;

mod proto;

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
pub enum ExplorerTransactions {
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
        ExplorerTransactions::tx_from_raw(raw).map(ExplorerTransactions::into)
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
    use exonum::blockchain::{GenesisConfig, ValidatorKeys};

    let (consensus_key, _) = consensus_keys();
    let service_keys = crypto::gen_keypair();

    let api_channel = mpsc::channel(10);
    let mut blockchain = Blockchain::new(
        TemporaryDB::new(),
        vec![MyService.into()],
        service_keys.0,
        service_keys.1,
        ApiSender(api_channel.0),
    );

    let keys = ValidatorKeys {
        consensus_key,
        service_key: service_keys.0,
    };
    blockchain
        .initialize(GenesisConfig::new(vec![keys].into_iter()))
        .unwrap();
    blockchain
}

/// Simplified compared to real life / testkit, but we don't need to test *everything*
/// here.
pub fn create_block(blockchain: &mut Blockchain, transactions: Vec<Signed<RawTransaction>>) {
    use exonum::helpers::{Round, ValidatorId};
    use exonum::messages::{Precommit, Propose};
    use std::time::SystemTime;

    let tx_hashes: Vec<_> = transactions.iter().map(Signed::hash).collect();
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
            &propose.hash(),
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
