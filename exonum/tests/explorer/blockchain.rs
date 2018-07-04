// Copyright 2018 The Exonum Team
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

extern crate futures;

use self::futures::sync::mpsc;
use exonum::{
    blockchain::{
        Blockchain, ExecutionError, ExecutionResult, Schema, Service, Transaction, TransactionSet,
    },
    crypto::{self, CryptoHash, Hash, PublicKey, SecretKey}, encoding::Error as EncodingError,
    messages::RawTransaction, node::ApiSender, storage::{Fork, MemoryDB, Snapshot},
};

transactions! {
    Transactions {
        const SERVICE_ID = 10_000;

        struct CreateWallet {
            pubkey: &PublicKey,
            name: &str,
        }

        struct Transfer {
            from: &PublicKey,
            to: &PublicKey,
            amount: u64,
        }
    }
}

impl Transaction for CreateWallet {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, _: &mut Fork) -> ExecutionResult {
        if self.name().starts_with("Al") {
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
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, _: &mut Fork) -> ExecutionResult {
        panic!("oops")
    }
}

struct MyService;

impl Service for MyService {
    fn service_id(&self) -> u16 {
        10_000
    }

    fn service_name(&self) -> &str {
        "my-service"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        vec![]
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
        Transactions::tx_from_raw(raw).map(Transactions::into)
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
        MemoryDB::new(),
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
pub fn create_block(blockchain: &mut Blockchain, transactions: Vec<Box<Transaction>>) {
    use exonum::helpers::{Round, ValidatorId};
    use exonum::messages::{Precommit, Propose};
    use std::time::SystemTime;

    let tx_hashes: Vec<_> = transactions.iter().map(|tx| tx.hash()).collect();
    let height = blockchain.last_block().height().next();

    let mut fork = blockchain.fork();
    {
        let mut schema = Schema::new(&mut fork);
        for tx in transactions {
            schema.add_transaction_into_pool(tx.raw().clone())
        }
    }
    blockchain.merge(fork.into_patch()).unwrap();

    let (block_hash, patch) = blockchain.create_patch(ValidatorId(0), height, &tx_hashes);
    let (_, consensus_secret_key) = consensus_keys();

    let propose = Propose::new(
        ValidatorId(0),
        height,
        Round::first(),
        &blockchain.last_hash(),
        &tx_hashes,
        &consensus_secret_key,
    );
    let precommit = Precommit::new(
        ValidatorId(0),
        propose.height(),
        propose.round(),
        &propose.hash(),
        &block_hash,
        SystemTime::now().into(),
        &consensus_secret_key,
    );

    blockchain
        .commit(&patch, block_hash, [precommit].into_iter())
        .unwrap();
}
